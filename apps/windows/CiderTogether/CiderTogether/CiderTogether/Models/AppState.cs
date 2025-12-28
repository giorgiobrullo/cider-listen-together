using CommunityToolkit.Mvvm.ComponentModel;
using Microsoft.UI.Dispatching;
using uniffi.cider_core;
using Windows.Storage;

namespace CiderTogether.Models;

/// <summary>
/// View state enumeration matching the Swift implementation.
/// </summary>
public enum ViewState
{
    Home,
    Creating,
    Joining,
    InRoom
}

/// <summary>
/// Joining progress state for the joining flow.
/// </summary>
public enum JoiningProgress
{
    Searching,
    Connecting,
    Timeout
}

/// <summary>
/// Main application state, matching the Swift AppState implementation.
/// Uses CommunityToolkit.Mvvm for observable properties.
/// </summary>
public partial class AppState : ObservableObject
{
    private readonly Session _session;
    private readonly DispatcherQueue _dispatcherQueue;
    private CancellationTokenSource? _pollingCts;
    private int _consecutiveFailures;
    private const int MaxConsecutiveFailures = 5;
    private bool _hasAppeared;

    // Observable properties
    [ObservableProperty] private ViewState _viewState = ViewState.Home;
    [ObservableProperty] private JoiningProgress _joiningProgress = JoiningProgress.Searching;
    [ObservableProperty] private bool _ciderConnected;
    [ObservableProperty] private bool _isCheckingConnection;
    [ObservableProperty] private RoomState? _roomState;
    [ObservableProperty] private TrackInfo? _nowPlaying;
    [ObservableProperty] private PlaybackState? _playback;
    [ObservableProperty] private string? _errorMessage;
    [ObservableProperty] private string? _connectionError;
    [ObservableProperty] private bool _ciderDisconnected;
    [ObservableProperty] private bool _isPlaying;
    [ObservableProperty] private bool _isHost;
    [ObservableProperty] private bool _isInRoom;
    [ObservableProperty] private string? _joiningRoomCode;
    [ObservableProperty] private SyncStatus? _syncStatus;

    // Persisted settings
    public string DisplayName
    {
        get => ApplicationData.Current.LocalSettings.Values["DisplayName"] as string ?? "Listener";
        set
        {
            if (DisplayName != value)
            {
                ApplicationData.Current.LocalSettings.Values["DisplayName"] = value;
                OnPropertyChanged();
            }
        }
    }

    public string ApiToken
    {
        get => ApplicationData.Current.LocalSettings.Values["ApiToken"] as string ?? "";
        set
        {
            if (ApiToken != value)
            {
                ApplicationData.Current.LocalSettings.Values["ApiToken"] = value;
                OnPropertyChanged();
            }
        }
    }

    public AppState()
    {
        _dispatcherQueue = DispatcherQueue.GetForCurrentThread();
        _session = new Session();
        _session.SetCallback(new SessionCallbackImpl(this, _dispatcherQueue));

        // Apply saved token (don't throw if it fails)
        try
        {
            if (!string.IsNullOrEmpty(ApiToken))
            {
                _session.SetCiderToken(ApiToken);
            }
        }
        catch
        {
            // Ignore - will be handled when checking connection
        }
    }

    /// <summary>
    /// Called when the main view appears.
    /// </summary>
    public async Task OnAppearAsync()
    {
        if (_hasAppeared) return;
        _hasAppeared = true;

        await CheckCiderConnectionAsync(showError: false);
    }

    // Settings methods

    public void UpdateApiToken(string token)
    {
        ApiToken = token;
        _session.SetCiderToken(string.IsNullOrEmpty(token) ? null : token);
        ConnectionError = null;
    }

    // Connection methods

    public async Task CheckCiderConnectionAsync(bool showError = true)
    {
        IsCheckingConnection = true;
        ConnectionError = null;
        var startTime = DateTime.Now;

        try
        {
            await Task.Run(() => _session.CheckCiderConnection());

            CiderConnected = true;
            ConnectionError = null;
            CiderDisconnected = false;
            _consecutiveFailures = 0;

            await FetchNowPlayingAsync();
            StartPolling();
        }
        catch (CoreException.CiderNotReachable)
        {
            CiderConnected = false;
            if (showError)
                ConnectionError = "Cider is not running or not reachable";
            StopPolling();
        }
        catch (CoreException.CiderApiException ex)
        {
            CiderConnected = false;
            if (showError)
                ConnectionError = ex.v1;
            StopPolling();
        }
        catch (CoreException.NetworkException ex)
        {
            CiderConnected = false;
            if (showError)
                ConnectionError = $"Network error: {ex.v1}";
            StopPolling();
        }
        catch (Exception ex)
        {
            CiderConnected = false;
            if (showError)
                ConnectionError = ex.Message;
            StopPolling();
        }
        finally
        {
            // Ensure loading is visible for at least 200ms
            var elapsed = DateTime.Now - startTime;
            if (elapsed.TotalMilliseconds < 200)
            {
                await Task.Delay(200 - (int)elapsed.TotalMilliseconds);
            }
            IsCheckingConnection = false;
        }
    }

    private async Task<bool> FetchNowPlayingAsync()
    {
        try
        {
            var playback = await Task.Run(() => _session.GetPlaybackState());

            NowPlaying = playback.track;
            IsPlaying = playback.isPlaying;
            _consecutiveFailures = 0;

            if (CiderDisconnected)
                CiderDisconnected = false;

            return true;
        }
        catch
        {
            _consecutiveFailures++;
            if (_consecutiveFailures >= 3)
                CiderDisconnected = true;

            if (_consecutiveFailures >= MaxConsecutiveFailures)
            {
                NowPlaying = null;
                IsPlaying = false;
            }

            return false;
        }
    }

    private void StartPolling()
    {
        StopPolling();
        _pollingCts = new CancellationTokenSource();

        _ = Task.Run(async () =>
        {
            var token = _pollingCts.Token;
            while (!token.IsCancellationRequested)
            {
                var success = await FetchNowPlayingAsync();
                var delay = success ? 1500 : 3000;

                try
                {
                    await Task.Delay(delay, token);
                }
                catch (TaskCanceledException)
                {
                    break;
                }
            }
        });
    }

    private void StopPolling()
    {
        _pollingCts?.Cancel();
        _pollingCts?.Dispose();
        _pollingCts = null;
    }

    // Room management

    public void CreateRoom()
    {
        ViewState = ViewState.Creating;
        var name = DisplayName;

        _ = Task.Run(async () =>
        {
            try
            {
                var code = _session.CreateRoom(name);

                _dispatcherQueue.TryEnqueue(() =>
                {
                    ViewState = ViewState.InRoom;
                    IsInRoom = true;
                    IsHost = true;
                });
            }
            catch (Exception ex)
            {
                _dispatcherQueue.TryEnqueue(() =>
                {
                    ErrorMessage = $"Failed to create room: {ex.Message}";
                    ViewState = ViewState.Home;
                });
            }
        });
    }

    public void JoinRoom(string code)
    {
        ViewState = ViewState.Joining;
        JoiningProgress = JoiningProgress.Searching;
        JoiningRoomCode = code;
        var name = DisplayName;

        _ = Task.Run(() =>
        {
            try
            {
                _session.JoinRoom(code, name);
                // Success is handled by callbacks (OnConnected, OnRoomStateChanged)
            }
            catch (Exception ex)
            {
                _dispatcherQueue.TryEnqueue(() =>
                {
                    ErrorMessage = $"Failed to join room: {ex.Message}";
                    ViewState = ViewState.Home;
                    JoiningRoomCode = null;
                });
            }
        });
    }

    public void CancelJoin()
    {
        ViewState = ViewState.Home;
        JoiningRoomCode = null;
        ErrorMessage = null;

        _ = Task.Run(() =>
        {
            try
            {
                _session.LeaveRoom();
            }
            catch
            {
                // Ignore errors when canceling
            }
        });
    }

    public void RetryJoin()
    {
        if (JoiningRoomCode != null)
        {
            JoinRoom(JoiningRoomCode);
        }
    }

    public void LeaveRoom()
    {
        _ = Task.Run(async () =>
        {
            try
            {
                _session.LeaveRoom();

                _dispatcherQueue.TryEnqueue(() =>
                {
                    ViewState = ViewState.Home;
                    RoomState = null;
                    IsInRoom = false;
                    IsHost = false;
                });
            }
            catch (Exception ex)
            {
                _dispatcherQueue.TryEnqueue(() =>
                {
                    ErrorMessage = $"Failed to leave room: {ex.Message}";
                });
            }
        });
    }

    public void TransferHost(string peerId)
    {
        _ = Task.Run(() =>
        {
            try
            {
                _session.TransferHost(peerId);
            }
            catch (Exception ex)
            {
                _dispatcherQueue.TryEnqueue(() =>
                {
                    ErrorMessage = $"Failed to transfer host: {ex.Message}";
                });
            }
        });
    }

    // Playback controls

    public void Play()
    {
        _ = Task.Run(() =>
        {
            try { _session.SyncPlay(); }
            catch { /* Ignore */ }
        });
    }

    public void Pause()
    {
        _ = Task.Run(() =>
        {
            try { _session.SyncPause(); }
            catch { /* Ignore */ }
        });
    }

    public void Next()
    {
        _ = Task.Run(() =>
        {
            try { _session.SyncNext(); }
            catch { /* Ignore */ }
        });
    }

    public void Previous()
    {
        _ = Task.Run(() =>
        {
            try { _session.SyncPrevious(); }
            catch { /* Ignore */ }
        });
    }

    // Internal methods for callback handling

    internal void HandleRoomStateChanged(RoomState state)
    {
        RoomState = state;
        IsInRoom = true;
        IsHost = state.localPeerId == state.hostPeerId;

        if (ViewState == ViewState.Joining && JoiningProgress == JoiningProgress.Searching)
        {
            JoiningProgress = JoiningProgress.Connecting;
        }
    }

    internal void HandleTrackChanged(TrackInfo? track)
    {
        NowPlaying = track;
        // Also update roomState.currentTrack for UI (like macOS does)
        if (RoomState != null)
        {
            RoomState = new RoomState(
                roomCode: RoomState.roomCode,
                hostPeerId: RoomState.hostPeerId,
                localPeerId: RoomState.localPeerId,
                participants: RoomState.participants,
                currentTrack: track,
                playback: RoomState.playback
            );
        }
    }

    internal void HandlePlaybackChanged(PlaybackState playback)
    {
        Playback = playback;
        // Note: Do NOT update IsPlaying here - that comes from local Cider polling only
        // Also update roomState.playback for UI (like macOS does)
        if (RoomState != null)
        {
            RoomState = new RoomState(
                roomCode: RoomState.roomCode,
                hostPeerId: RoomState.hostPeerId,
                localPeerId: RoomState.localPeerId,
                participants: RoomState.participants,
                currentTrack: RoomState.currentTrack,
                playback: playback
            );
        }
    }

    internal void HandleRoomEnded(string reason)
    {
        ErrorMessage = reason;
        ViewState = ViewState.Home;
        RoomState = null;
        IsInRoom = false;
        IsHost = false;
        JoiningRoomCode = null;
    }

    internal void HandleError(string message)
    {
        if (ViewState == ViewState.Joining && message.Contains("not found"))
        {
            JoiningProgress = JoiningProgress.Timeout;
            return;
        }

        ErrorMessage = message;
    }

    internal void HandleConnected()
    {
        if (ViewState == ViewState.Joining)
        {
            ViewState = ViewState.InRoom;
            JoiningRoomCode = null;
        }
    }

    internal void HandleDisconnected()
    {
        ViewState = ViewState.Home;
        RoomState = null;
        IsInRoom = false;
        IsHost = false;
        JoiningRoomCode = null;
        SyncStatus = null;
    }

    internal void HandleSyncStatus(SyncStatus status)
    {
        SyncStatus = status;
    }
}
