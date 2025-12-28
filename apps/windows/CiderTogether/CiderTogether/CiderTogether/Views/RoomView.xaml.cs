using Microsoft.UI;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Shapes;
using Windows.ApplicationModel.DataTransfer;
using CiderTogether.Models;
using uniffi.cider_core;

namespace CiderTogether.Views;

/// <summary>
/// Room view showing current room state, participants, and playback controls.
/// </summary>
public sealed partial class RoomView : Page
{
    private readonly AppState _appState;
    private bool _copyFeedbackShown;

    public RoomView()
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
                case nameof(AppState.CiderDisconnected):
                    CiderDisconnectedBanner.IsOpen = _appState.CiderDisconnected;
                    break;
                case nameof(AppState.RoomState):
                    UpdateRoomState();
                    break;
                case nameof(AppState.IsHost):
                    UpdatePlaybackControls();
                    break;
                case nameof(AppState.IsPlaying):
                    UpdatePlayPauseIcon();
                    break;
                case nameof(AppState.ErrorMessage):
                    ShowErrorIfNeeded();
                    break;
            }
        });
    }

    private void UpdateUIState()
    {
        CiderDisconnectedBanner.IsOpen = _appState.CiderDisconnected;
        UpdateRoomState();
        UpdatePlaybackControls();
        UpdatePlayPauseIcon();
    }

    private void UpdateRoomState()
    {
        var roomState = _appState.RoomState;
        if (roomState == null) return;

        // Update room code
        RoomCodeText.Text = FormatRoomCode(roomState.roomCode);

        // Update participants
        ParticipantsHeader.Text = $"Listening ({roomState.participants.Length})";
        UpdateParticipantsList(roomState.participants);
    }

    private void UpdateParticipantsList(Participant[] participants)
    {
        var badges = new List<Border>();
        foreach (var participant in participants)
        {
            badges.Add(CreateParticipantBadge(participant));
        }
        ParticipantsPanel.ItemsSource = badges;
    }

    private Border CreateParticipantBadge(Participant participant)
    {
        var border = new Border
        {
            Background = (Brush)Application.Current.Resources["CardBackgroundFillColorDefaultBrush"],
            BorderBrush = (Brush)Application.Current.Resources["CardStrokeColorDefaultBrush"],
            BorderThickness = new Thickness(1),
            CornerRadius = new CornerRadius(16),
            Padding = new Thickness(12, 6, 12, 6)
        };

        var stack = new StackPanel
        {
            Orientation = Orientation.Horizontal,
            Spacing = 6
        };

        // Avatar with initials
        var avatarGrid = new Grid
        {
            Width = 22,
            Height = 22
        };

        var avatarEllipse = new Ellipse
        {
            Width = 22,
            Height = 22,
            Fill = new SolidColorBrush(GetAvatarColor(participant.displayName))
        };

        var initialsText = new TextBlock
        {
            Text = GetInitials(participant.displayName),
            FontSize = 9,
            FontWeight = Microsoft.UI.Text.FontWeights.SemiBold,
            Foreground = new SolidColorBrush(Colors.White),
            HorizontalAlignment = HorizontalAlignment.Center,
            VerticalAlignment = VerticalAlignment.Center
        };

        avatarGrid.Children.Add(avatarEllipse);
        avatarGrid.Children.Add(initialsText);
        stack.Children.Add(avatarGrid);

        // Name
        var nameText = new TextBlock
        {
            Text = participant.displayName,
            Style = (Style)Application.Current.Resources["CaptionTextBlockStyle"],
            VerticalAlignment = VerticalAlignment.Center,
            MaxLines = 1
        };
        stack.Children.Add(nameText);

        // Host badge
        if (participant.isHost)
        {
            var hostIcon = new FontIcon
            {
                Glyph = "\uE735", // Star
                FontSize = 10,
                Foreground = new SolidColorBrush(Colors.Orange)
            };
            stack.Children.Add(hostIcon);
        }

        border.Child = stack;
        return border;
    }

    private static string GetInitials(string name)
    {
        var words = name.Split(' ', StringSplitOptions.RemoveEmptyEntries);
        if (words.Length >= 2)
        {
            return $"{words[0][0]}{words[1][0]}".ToUpper();
        }
        return name.Length >= 2 ? name[..2].ToUpper() : name.ToUpper();
    }

    private static Windows.UI.Color GetAvatarColor(string name)
    {
        // Generate consistent color from name hash
        var hash = name.GetHashCode();
        var hue = Math.Abs(hash) % 360;

        // Convert HSL to RGB (saturation=0.5, lightness=0.6)
        return HslToColor(hue, 0.5, 0.6);
    }

    private static Windows.UI.Color HslToColor(double h, double s, double l)
    {
        double c = (1 - Math.Abs(2 * l - 1)) * s;
        double x = c * (1 - Math.Abs((h / 60) % 2 - 1));
        double m = l - c / 2;

        double r, g, b;
        if (h < 60) { r = c; g = x; b = 0; }
        else if (h < 120) { r = x; g = c; b = 0; }
        else if (h < 180) { r = 0; g = c; b = x; }
        else if (h < 240) { r = 0; g = x; b = c; }
        else if (h < 300) { r = x; g = 0; b = c; }
        else { r = c; g = 0; b = x; }

        return Windows.UI.Color.FromArgb(
            255,
            (byte)((r + m) * 255),
            (byte)((g + m) * 255),
            (byte)((b + m) * 255)
        );
    }

    private void UpdatePlaybackControls()
    {
        PlaybackControls.Visibility = _appState.IsHost
            ? Visibility.Visible
            : Visibility.Collapsed;
    }

    private void UpdatePlayPauseIcon()
    {
        PlayPauseIcon.Glyph = _appState.IsPlaying ? "\uE769" : "\uE768"; // Pause : Play
    }

    private static string FormatRoomCode(string code)
    {
        if (code.Length == 8)
        {
            return $"{code[..4]}-{code[4..]}";
        }
        return code;
    }

    private async void ShowErrorIfNeeded()
    {
        if (!string.IsNullOrEmpty(_appState.ErrorMessage))
        {
            ErrorDialog.Content = _appState.ErrorMessage;
            await ErrorDialog.ShowAsync();
            _appState.ErrorMessage = null;
        }
    }

    private async void CopyRoomCode_Click(object sender, RoutedEventArgs e)
    {
        var roomState = _appState.RoomState;
        if (roomState == null) return;

        var dataPackage = new DataPackage();
        dataPackage.SetText(FormatRoomCode(roomState.roomCode));
        Clipboard.SetContent(dataPackage);

        // Show feedback
        if (!_copyFeedbackShown)
        {
            _copyFeedbackShown = true;
            CopyIcon.Glyph = "\uE73E"; // Checkmark

            await Task.Delay(2000);

            CopyIcon.Glyph = "\uE8C8"; // Copy
            _copyFeedbackShown = false;
        }
    }

    private async void RetryConnection_Click(object sender, RoutedEventArgs e)
    {
        await _appState.CheckCiderConnectionAsync();
    }

    private void PlayPause_Click(object sender, RoutedEventArgs e)
    {
        if (_appState.IsPlaying)
        {
            _appState.Pause();
        }
        else
        {
            _appState.Play();
        }
    }

    private void Previous_Click(object sender, RoutedEventArgs e)
    {
        _appState.Previous();
    }

    private void Next_Click(object sender, RoutedEventArgs e)
    {
        _appState.Next();
    }
}
