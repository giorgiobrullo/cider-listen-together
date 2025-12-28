using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media.Imaging;
using CiderTogether.Models;
using uniffi.cider_core;

namespace CiderTogether.Controls;

/// <summary>
/// Now playing card that displays current track info and progress.
/// </summary>
public sealed partial class NowPlayingCard : UserControl
{
    private readonly AppState _appState;
    private readonly DispatcherTimer _progressTimer;
    private DateTime _lastUpdateTime;
    private ulong _lastPositionMs;
    private string? _currentArtworkUrl;

    public NowPlayingCard()
    {
        this.InitializeComponent();
        _appState = App.AppState;

        // Subscribe to state changes
        _appState.PropertyChanged += AppState_PropertyChanged;

        // Set up progress timer for smooth updates
        _progressTimer = new DispatcherTimer
        {
            Interval = TimeSpan.FromMilliseconds(250)
        };
        _progressTimer.Tick += ProgressTimer_Tick;

        // Initial update
        UpdateTrackInfo();
    }

    private void AppState_PropertyChanged(object? sender, System.ComponentModel.PropertyChangedEventArgs e)
    {
        DispatcherQueue.TryEnqueue(() =>
        {
            switch (e.PropertyName)
            {
                case nameof(AppState.NowPlaying):
                case nameof(AppState.IsPlaying):
                case nameof(AppState.IsInRoom):
                case nameof(AppState.IsHost):
                case nameof(AppState.RoomState):
                    UpdateTrackInfo();
                    break;
            }
        });
    }

    private TrackInfo? GetDisplayTrack()
    {
        // If in room as listener, use room state's track from host
        if (_appState.IsInRoom && !_appState.IsHost)
        {
            return _appState.RoomState?.currentTrack;
        }
        // Otherwise use local Cider playback
        return _appState.NowPlaying;
    }

    private bool GetDisplayIsPlaying()
    {
        // If in room as listener, use room state's playback from host
        if (_appState.IsInRoom && !_appState.IsHost)
        {
            return _appState.RoomState?.playback?.isPlaying ?? false;
        }
        // Otherwise use local state
        return _appState.IsPlaying;
    }

    private ulong GetDisplayPositionMs(TrackInfo track)
    {
        // If in room as listener, use room state's playback position from host
        if (_appState.IsInRoom && !_appState.IsHost && _appState.RoomState?.playback != null)
        {
            return _appState.RoomState.playback.positionMs;
        }
        // Otherwise use track's position
        return track.positionMs;
    }

    private void UpdateTrackInfo()
    {
        var track = GetDisplayTrack();

        if (track != null)
        {
            // Show track info
            TrackInfoPanel.Visibility = Visibility.Visible;
            NoTrackPanel.Visibility = Visibility.Collapsed;

            TrackNameText.Text = track.name;
            ArtistNameText.Text = track.artist;

            // Update artwork only if URL changed
            if (track.artworkUrl != _currentArtworkUrl)
            {
                _currentArtworkUrl = track.artworkUrl;
                if (!string.IsNullOrEmpty(track.artworkUrl))
                {
                    try
                    {
                        ArtworkImage.Source = new BitmapImage(new Uri(track.artworkUrl));
                        PlaceholderIcon.Visibility = Visibility.Collapsed;
                    }
                    catch
                    {
                        ArtworkImage.Source = null;
                        PlaceholderIcon.Visibility = Visibility.Visible;
                    }
                }
                else
                {
                    ArtworkImage.Source = null;
                    PlaceholderIcon.Visibility = Visibility.Visible;
                }
            }

            // Update progress
            var positionMs = GetDisplayPositionMs(track);
            UpdateProgress(track, positionMs);

            // Start/stop progress timer based on correct playing state
            var isPlaying = GetDisplayIsPlaying();
            if (isPlaying)
            {
                _lastUpdateTime = DateTime.Now;
                _lastPositionMs = positionMs;
                _progressTimer.Start();
            }
            else
            {
                _progressTimer.Stop();
            }
        }
        else
        {
            // Show no track
            TrackInfoPanel.Visibility = Visibility.Collapsed;
            NoTrackPanel.Visibility = Visibility.Visible;
            ArtworkImage.Source = null;
            PlaceholderIcon.Visibility = Visibility.Visible;
            _currentArtworkUrl = null;
            _progressTimer.Stop();

            // Update subtext
            NoTrackSubtext.Text = _appState.IsInRoom && !_appState.IsHost
                ? "Waiting for host..."
                : "Play something in Cider";
        }
    }

    private void UpdateProgress(TrackInfo track, ulong positionMs)
    {
        var durationMs = track.durationMs;

        // Calculate progress percentage
        double progress = durationMs > 0 ? (double)positionMs / durationMs * 100 : 0;
        TrackProgress.Value = Math.Min(progress, 100);

        // Format times
        CurrentTimeText.Text = FormatTime(positionMs);
        DurationText.Text = FormatTime(durationMs);

        // Store for interpolation
        _lastPositionMs = positionMs;
        _lastUpdateTime = DateTime.Now;
    }

    private void ProgressTimer_Tick(object? sender, object e)
    {
        var track = GetDisplayTrack();
        var isPlaying = GetDisplayIsPlaying();
        if (track == null || !isPlaying)
        {
            _progressTimer.Stop();
            return;
        }

        // Interpolate position
        var elapsed = DateTime.Now - _lastUpdateTime;
        var interpolatedPosition = _lastPositionMs + (ulong)elapsed.TotalMilliseconds;
        var durationMs = track.durationMs;

        // Clamp to duration
        interpolatedPosition = Math.Min(interpolatedPosition, durationMs);

        // Update progress
        double progress = durationMs > 0 ? (double)interpolatedPosition / durationMs * 100 : 0;
        TrackProgress.Value = Math.Min(progress, 100);
        CurrentTimeText.Text = FormatTime(interpolatedPosition);
    }

    private static string FormatTime(ulong ms)
    {
        var totalSeconds = ms / 1000;
        var minutes = totalSeconds / 60;
        var seconds = totalSeconds % 60;
        return $"{minutes}:{seconds:D2}";
    }
}
