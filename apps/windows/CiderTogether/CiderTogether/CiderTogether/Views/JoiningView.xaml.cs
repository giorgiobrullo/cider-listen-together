using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using CiderTogether.Models;

namespace CiderTogether.Views;

/// <summary>
/// View shown while creating or joining a room.
/// </summary>
public sealed partial class JoiningView : Page
{
    private readonly AppState _appState;

    public JoiningView()
    {
        this.InitializeComponent();
        _appState = App.AppState;

        // Subscribe to state changes
        _appState.PropertyChanged += AppState_PropertyChanged;

        // Initial update
        UpdateUIState();
    }

    private void AppState_PropertyChanged(object? sender, System.ComponentModel.PropertyChangedEventArgs e)
    {
        DispatcherQueue.TryEnqueue(() =>
        {
            switch (e.PropertyName)
            {
                case nameof(AppState.ViewState):
                case nameof(AppState.JoiningProgress):
                case nameof(AppState.JoiningRoomCode):
                    UpdateUIState();
                    break;
            }
        });
    }

    private void UpdateUIState()
    {
        // Hide all panels first
        CreatingPanel.Visibility = Visibility.Collapsed;
        SearchingPanel.Visibility = Visibility.Collapsed;
        ConnectingPanel.Visibility = Visibility.Collapsed;
        TimeoutPanel.Visibility = Visibility.Collapsed;

        if (_appState.ViewState == ViewState.Creating)
        {
            CreatingPanel.Visibility = Visibility.Visible;
        }
        else if (_appState.ViewState == ViewState.Joining)
        {
            switch (_appState.JoiningProgress)
            {
                case JoiningProgress.Searching:
                    SearchingPanel.Visibility = Visibility.Visible;
                    if (_appState.JoiningRoomCode != null)
                    {
                        RoomCodeText.Text = $"Room code: {FormatRoomCode(_appState.JoiningRoomCode)}";
                    }
                    break;

                case JoiningProgress.Connecting:
                    ConnectingPanel.Visibility = Visibility.Visible;
                    break;

                case JoiningProgress.Timeout:
                    TimeoutPanel.Visibility = Visibility.Visible;
                    if (_appState.JoiningRoomCode != null)
                    {
                        TimeoutRoomCodeText.Text = $"Could not find room {FormatRoomCode(_appState.JoiningRoomCode)}";
                    }
                    break;
            }
        }
    }

    private static string FormatRoomCode(string code)
    {
        if (code.Length == 8)
        {
            return $"{code[..4]}-{code[4..]}";
        }
        return code;
    }

    private void Cancel_Click(object sender, RoutedEventArgs e)
    {
        _appState.CancelJoin();
    }

    private void GoBack_Click(object sender, RoutedEventArgs e)
    {
        _appState.CancelJoin();
    }

    private void Retry_Click(object sender, RoutedEventArgs e)
    {
        _appState.RetryJoin();
    }
}
