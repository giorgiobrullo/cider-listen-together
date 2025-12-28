using Microsoft.UI.Dispatching;
using uniffi.cider_core;

namespace CiderTogether.Models;

/// <summary>
/// Implementation of the SessionCallback interface.
/// Marshals all callbacks to the UI thread.
/// </summary>
internal class SessionCallbackImpl : SessionCallback
{
    private readonly WeakReference<AppState> _appStateRef;
    private readonly DispatcherQueue _dispatcher;

    public SessionCallbackImpl(AppState appState, DispatcherQueue dispatcher)
    {
        _appStateRef = new WeakReference<AppState>(appState);
        _dispatcher = dispatcher;
    }

    public void OnRoomStateChanged(RoomState state)
    {
        _dispatcher.TryEnqueue(() =>
        {
            if (_appStateRef.TryGetTarget(out var appState))
            {
                appState.HandleRoomStateChanged(state);
            }
        });
    }

    public void OnTrackChanged(TrackInfo? track)
    {
        _dispatcher.TryEnqueue(() =>
        {
            if (_appStateRef.TryGetTarget(out var appState))
            {
                appState.HandleTrackChanged(track);
            }
        });
    }

    public void OnPlaybackChanged(PlaybackState playback)
    {
        _dispatcher.TryEnqueue(() =>
        {
            if (_appStateRef.TryGetTarget(out var appState))
            {
                appState.HandlePlaybackChanged(playback);
            }
        });
    }

    public void OnParticipantJoined(Participant participant)
    {
        // Room state will be updated via OnRoomStateChanged
    }

    public void OnParticipantLeft(string peerId)
    {
        // Room state will be updated via OnRoomStateChanged
    }

    public void OnRoomEnded(string reason)
    {
        _dispatcher.TryEnqueue(() =>
        {
            if (_appStateRef.TryGetTarget(out var appState))
            {
                appState.HandleRoomEnded(reason);
            }
        });
    }

    public void OnError(string message)
    {
        _dispatcher.TryEnqueue(() =>
        {
            if (_appStateRef.TryGetTarget(out var appState))
            {
                appState.HandleError(message);
            }
        });
    }

    public void OnConnected()
    {
        _dispatcher.TryEnqueue(() =>
        {
            if (_appStateRef.TryGetTarget(out var appState))
            {
                appState.HandleConnected();
            }
        });
    }

    public void OnDisconnected()
    {
        _dispatcher.TryEnqueue(() =>
        {
            if (_appStateRef.TryGetTarget(out var appState))
            {
                appState.HandleDisconnected();
            }
        });
    }

    public void OnSyncStatus(SyncStatus status)
    {
        _dispatcher.TryEnqueue(() =>
        {
            if (_appStateRef.TryGetTarget(out var appState))
            {
                appState.HandleSyncStatus(status);
            }
        });
    }
}
