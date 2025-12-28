using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using CiderTogether.Models;

namespace CiderTogether.Views;

/// <summary>
/// Home view for connecting to Cider and creating/joining rooms.
/// </summary>
public sealed partial class HomeView : Page
{
    private readonly AppState _appState;

    public HomeView()
    {
        this.InitializeComponent();
        _appState = App.AppState;

        // Subscribe to state changes
        _appState.PropertyChanged += AppState_PropertyChanged;

        // Initialize UI state
        UpdateUIState();
        LoadSettings();
    }

    private void LoadSettings()
    {
        DisplayNameTextBox.Text = _appState.DisplayName;
        SetupDisplayNameBox.Text = _appState.DisplayName;
        ApiTokenBox.Password = _appState.ApiToken;
    }

    private void AppState_PropertyChanged(object? sender, System.ComponentModel.PropertyChangedEventArgs e)
    {
        DispatcherQueue.TryEnqueue(() =>
        {
            switch (e.PropertyName)
            {
                case nameof(AppState.CiderConnected):
                case nameof(AppState.IsCheckingConnection):
                case nameof(AppState.CiderDisconnected):
                case nameof(AppState.ConnectionError):
                    UpdateUIState();
                    break;
            }
        });
    }

    private void UpdateUIState()
    {
        bool connected = _appState.CiderConnected;
        bool checking = _appState.IsCheckingConnection;

        // Show/hide main panels
        ConnectedPanel.Visibility = connected ? Visibility.Visible : Visibility.Collapsed;
        DisconnectedPanel.Visibility = connected ? Visibility.Collapsed : Visibility.Visible;

        // Update checking state
        CheckingConnectionPanel.Visibility = checking ? Visibility.Visible : Visibility.Collapsed;
        NotConnectedPanel.Visibility = checking ? Visibility.Collapsed : Visibility.Visible;

        // Update warning banner
        CiderWarningBanner.IsOpen = _appState.CiderDisconnected;

        // Update connection error
        if (!string.IsNullOrEmpty(_appState.ConnectionError))
        {
            ConnectionErrorText.Text = _appState.ConnectionError;
            ConnectionErrorText.Visibility = Visibility.Visible;
        }
        else
        {
            ConnectionErrorText.Visibility = Visibility.Collapsed;
        }

        // Update title for disconnected state
        if (_appState.CiderDisconnected)
        {
            ConnectionTitle.Text = "Cider Disconnected";
            ConnectionSubtitle.Text = "Cider was closed or stopped responding. Restart it and reconnect.";
            ConnectionSubtitle.Foreground = new Microsoft.UI.Xaml.Media.SolidColorBrush(Microsoft.UI.Colors.Orange);
        }
        else
        {
            ConnectionTitle.Text = "Connect to Cider";
            ConnectionSubtitle.Text = "Make sure Cider is running with API access enabled.";
            ConnectionSubtitle.Foreground = (Microsoft.UI.Xaml.Media.Brush)Application.Current.Resources["TextFillColorSecondaryBrush"];
        }
    }

    // Event handlers

    private async void Connect_Click(object sender, RoutedEventArgs e)
    {
        await _appState.CheckCiderConnectionAsync();
    }

    private async void RetryConnection_Click(object sender, RoutedEventArgs e)
    {
        await _appState.CheckCiderConnectionAsync();
    }

    private void CreateRoom_Click(object sender, RoutedEventArgs e)
    {
        _appState.CreateRoom();
    }

    private async void JoinRoom_Click(object sender, RoutedEventArgs e)
    {
        RoomCodeTextBox.Text = "";
        RoomCodeValidation.Visibility = Visibility.Collapsed;
        JoinRoomDialog.IsPrimaryButtonEnabled = false;
        await JoinRoomDialog.ShowAsync();
    }

    private void JoinRoomDialog_PrimaryButtonClick(ContentDialog sender, ContentDialogButtonClickEventArgs args)
    {
        var cleanCode = GetCleanRoomCode();
        if (cleanCode.Length == 8)
        {
            _appState.JoinRoom(cleanCode);
        }
        else
        {
            args.Cancel = true;
        }
    }

    private void RoomCode_TextChanged(object sender, TextChangedEventArgs e)
    {
        var text = RoomCodeTextBox.Text;
        var cleanCode = GetCleanRoomCode();

        // Format as XXXX-XXXX
        if (cleanCode.Length > 4 && !text.Contains('-'))
        {
            RoomCodeTextBox.Text = $"{cleanCode[..4]}-{cleanCode[4..]}";
            RoomCodeTextBox.SelectionStart = RoomCodeTextBox.Text.Length;
        }

        // Validate
        bool isValid = cleanCode.Length == 8;
        JoinRoomDialog.IsPrimaryButtonEnabled = isValid;
        RoomCodeValidation.Visibility = (text.Length > 0 && !isValid)
            ? Visibility.Visible
            : Visibility.Collapsed;
    }

    private string GetCleanRoomCode()
    {
        return RoomCodeTextBox.Text
            .Replace("-", "")
            .ToUpperInvariant()
            .Where(c => char.IsLetterOrDigit(c))
            .Take(8)
            .Aggregate("", (s, c) => s + c);
    }

    private async void Settings_Click(object sender, RoutedEventArgs e)
    {
        SettingsApiTokenBox.Password = _appState.ApiToken;
        SettingsDisplayNameBox.Text = _appState.DisplayName;
        await SettingsDialog.ShowAsync();
    }

    private void SettingsDialog_PrimaryButtonClick(ContentDialog sender, ContentDialogButtonClickEventArgs args)
    {
        _appState.UpdateApiToken(SettingsApiTokenBox.Password);
        _appState.DisplayName = SettingsDisplayNameBox.Text;
        DisplayNameTextBox.Text = SettingsDisplayNameBox.Text;
    }

    private void DisplayName_TextChanged(object sender, TextChangedEventArgs e)
    {
        _appState.DisplayName = DisplayNameTextBox.Text;
    }

    private void SetupDisplayName_TextChanged(object sender, TextChangedEventArgs e)
    {
        _appState.DisplayName = SetupDisplayNameBox.Text;
        DisplayNameTextBox.Text = SetupDisplayNameBox.Text;
    }

    private void ApiToken_PasswordChanged(object sender, RoutedEventArgs e)
    {
        _appState.UpdateApiToken(ApiTokenBox.Password);
    }
}
