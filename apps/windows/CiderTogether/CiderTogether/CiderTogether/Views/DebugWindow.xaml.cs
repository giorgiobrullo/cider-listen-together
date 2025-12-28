using CiderTogether.Models;
using Microsoft.UI;
using Microsoft.UI.Composition.SystemBackdrops;
using Microsoft.UI.Windowing;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;
using uniffi.cider_core;
using Windows.Graphics;
using WinRT.Interop;

namespace CiderTogether.Views;

public sealed partial class DebugWindow : Window
{
    private readonly AppState _appState;
    private AppWindow? _appWindow;

    // Window size in logical pixels
    private const int WindowWidth = 400;
    private const int WindowHeight = 600;

    public DebugWindow()
    {
        this.InitializeComponent();

        _appState = App.AppState;

        ConfigureWindow();
        ConfigureTitleBar();
        ConfigureBackdrop();

        // Subscribe to state changes
        _appState.PropertyChanged += AppState_PropertyChanged;

        // Initial update
        UpdateAllSections();
    }

    [System.Runtime.InteropServices.DllImport("user32.dll")]
    private static extern uint GetDpiForWindow(IntPtr hwnd);

    private void ConfigureWindow()
    {
        var hwnd = WindowNative.GetWindowHandle(this);
        var windowId = Win32Interop.GetWindowIdFromWindow(hwnd);
        _appWindow = AppWindow.GetFromWindowId(windowId);

        if (_appWindow != null)
        {
            // Get DPI scale factor
            var dpi = GetDpiForWindow(hwnd);
            var scaleFactor = dpi / 96.0;

            // Set size scaled for DPI
            var scaledWidth = (int)(WindowWidth * scaleFactor);
            var scaledHeight = (int)(WindowHeight * scaleFactor);
            _appWindow.Resize(new SizeInt32(scaledWidth, scaledHeight));

            _appWindow.Title = "Debug Info";
        }

        // Force dark theme
        if (Content is FrameworkElement root)
        {
            root.RequestedTheme = ElementTheme.Dark;
        }
    }

    private void ConfigureTitleBar()
    {
        ExtendsContentIntoTitleBar = true;
        SetTitleBar(AppTitleBar);
    }

    private void ConfigureBackdrop()
    {
        if (MicaController.IsSupported())
        {
            SystemBackdrop = new MicaBackdrop { Kind = MicaKind.Base };
        }
        else if (DesktopAcrylicController.IsSupported())
        {
            SystemBackdrop = new DesktopAcrylicBackdrop();
        }
    }

    private void AppState_PropertyChanged(object? sender, System.ComponentModel.PropertyChangedEventArgs e)
    {
        DispatcherQueue.TryEnqueue(() =>
        {
            switch (e.PropertyName)
            {
                case nameof(AppState.IsHost):
                case nameof(AppState.IsInRoom):
                case nameof(AppState.CiderConnected):
                case nameof(AppState.RoomState):
                    UpdateConnectionSection();
                    break;
                case nameof(AppState.SyncStatus):
                    UpdateSyncStatusSection();
                    break;
                case nameof(AppState.NowPlaying):
                case nameof(AppState.IsPlaying):
                    UpdateNowPlayingSection();
                    break;
            }
        });
    }

    private void UpdateAllSections()
    {
        UpdateConnectionSection();
        UpdateSyncStatusSection();
        UpdateNowPlayingSection();
    }

    private void UpdateConnectionSection()
    {
        RoleValue.Text = _appState.IsHost ? "Host" : "Listener";
        InRoomValue.Text = _appState.IsInRoom ? "Yes" : "No";
        CiderConnectedValue.Text = _appState.CiderConnected ? "Yes" : "No";

        var roomState = _appState.RoomState;
        if (roomState != null)
        {
            RoomCodeLabel.Visibility = Visibility.Visible;
            RoomCodeValue.Visibility = Visibility.Visible;
            RoomCodeValue.Text = roomState.roomCode;

            ParticipantsLabel.Visibility = Visibility.Visible;
            ParticipantsValue.Visibility = Visibility.Visible;
            ParticipantsValue.Text = roomState.participants.Length.ToString();

            LocalPeerLabel.Visibility = Visibility.Visible;
            LocalPeerValue.Visibility = Visibility.Visible;
            LocalPeerValue.Text = TruncatePeerId(roomState.localPeerId);

            HostPeerLabel.Visibility = Visibility.Visible;
            HostPeerValue.Visibility = Visibility.Visible;
            HostPeerValue.Text = TruncatePeerId(roomState.hostPeerId);
        }
        else
        {
            RoomCodeLabel.Visibility = Visibility.Collapsed;
            RoomCodeValue.Visibility = Visibility.Collapsed;
            ParticipantsLabel.Visibility = Visibility.Collapsed;
            ParticipantsValue.Visibility = Visibility.Collapsed;
            LocalPeerLabel.Visibility = Visibility.Collapsed;
            LocalPeerValue.Visibility = Visibility.Collapsed;
            HostPeerLabel.Visibility = Visibility.Collapsed;
            HostPeerValue.Visibility = Visibility.Collapsed;
        }
    }

    private void UpdateSyncStatusSection()
    {
        if (_appState.IsHost)
        {
            SyncStatusContent.Visibility = Visibility.Collapsed;
            SyncStatusWaiting.Visibility = Visibility.Collapsed;
            SyncStatusHostMessage.Visibility = Visibility.Visible;
            return;
        }

        var status = _appState.SyncStatus;
        if (status == null)
        {
            SyncStatusContent.Visibility = Visibility.Collapsed;
            SyncStatusWaiting.Visibility = Visibility.Visible;
            SyncStatusHostMessage.Visibility = Visibility.Collapsed;
            return;
        }

        SyncStatusContent.Visibility = Visibility.Visible;
        SyncStatusWaiting.Visibility = Visibility.Collapsed;
        SyncStatusHostMessage.Visibility = Visibility.Collapsed;

        // Drift
        DriftValue.Text = FormatDrift(status.driftMs);
        DriftValue.Foreground = new SolidColorBrush(GetDriftColor(status.driftMs));

        // Latency
        LatencyValue.Text = $"{status.latencyMs}ms";

        // Seek Offset
        SeekOffsetValue.Text = $"{status.seekOffsetMs}ms";

        // Calibration
        if (status.calibrationPending)
        {
            CalibrationSection.Visibility = Visibility.Visible;
            if (status.nextCalibrationSample.HasValue)
            {
                CalibrationFormula.Text = "ideal = offset - drift";
                var sign = status.driftMs >= 0 ? "+" : "";
                CalibrationValue.Text = $"{status.nextCalibrationSample.Value} = {status.seekOffsetMs} - ({sign}{status.driftMs})";
            }
            else
            {
                CalibrationFormula.Text = $"drift |{status.driftMs}ms| > 1500ms threshold";
                CalibrationValue.Text = "Sample will use damped weight (5%)";
            }
        }
        else
        {
            CalibrationSection.Visibility = Visibility.Collapsed;
        }

        // Last Heartbeat
        HeartbeatValue.Text = $"{status.elapsedMs}ms ago";

        // Quality
        var (qualityText, qualityColor) = GetQuality(status.driftMs);
        QualityValue.Text = qualityText;
        QualityValue.Foreground = new SolidColorBrush(qualityColor);
        QualityIndicator.Fill = new SolidColorBrush(qualityColor);

        // Calibration History
        if (status.sampleHistory.Length > 0)
        {
            CalibrationHistorySection.Visibility = Visibility.Visible;
            UpdateCalibrationHistory(status.sampleHistory);
        }
        else
        {
            CalibrationHistorySection.Visibility = Visibility.Collapsed;
        }
    }

    private void UpdateCalibrationHistory(CalibrationSample[] samples)
    {
        var items = new StackPanel { Spacing = 4 };

        // Show newest first
        for (int i = samples.Length - 1; i >= 0; i--)
        {
            var sample = samples[i];
            var isNewest = i == samples.Length - 1;

            var row = new Grid
            {
                Padding = new Thickness(6, 2, 6, 2),
                CornerRadius = new CornerRadius(4),
                Background = isNewest
                    ? new SolidColorBrush(Windows.UI.Color.FromArgb(0x1A, 0x00, 0x78, 0xD4))
                    : new SolidColorBrush(Colors.Transparent)
            };

            row.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(50) });
            row.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
            row.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
            row.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
            row.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });

            // Drift value
            var driftText = new TextBlock
            {
                Text = sample.driftMs >= 0 ? $"+{sample.driftMs}" : sample.driftMs.ToString(),
                FontFamily = new FontFamily("Consolas"),
                FontSize = 11,
                Foreground = new SolidColorBrush(sample.rejected ? Colors.Red : (sample.driftMs >= 0 ? Colors.Orange : Colors.DodgerBlue)),
                HorizontalAlignment = HorizontalAlignment.Right
            };
            Grid.SetColumn(driftText, 0);
            row.Children.Add(driftText);

            // Arrow
            var arrow = new FontIcon
            {
                Glyph = "\uE72A",
                FontSize = 10,
                Foreground = new SolidColorBrush(Windows.UI.Color.FromArgb(0x99, 0xFF, 0xFF, 0xFF)),
                Margin = new Thickness(6, 0, 6, 0)
            };
            Grid.SetColumn(arrow, 1);
            row.Children.Add(arrow);

            // New offset
            var offsetText = new TextBlock
            {
                Text = $"{sample.newOffsetMs}ms",
                FontFamily = new FontFamily("Consolas"),
                FontSize = 11,
                Foreground = new SolidColorBrush(sample.rejected
                    ? Windows.UI.Color.FromArgb(0x99, 0xFF, 0xFF, 0xFF)
                    : Colors.White)
            };
            Grid.SetColumn(offsetText, 2);
            row.Children.Add(offsetText);

            // Damped badge
            if (sample.rejected)
            {
                var dampedBadge = new Border
                {
                    Background = new SolidColorBrush(Windows.UI.Color.FromArgb(0x26, 0xFF, 0x98, 0x00)),
                    CornerRadius = new CornerRadius(3),
                    Padding = new Thickness(4, 1, 4, 1),
                    Margin = new Thickness(6, 0, 0, 0)
                };
                dampedBadge.Child = new TextBlock
                {
                    Text = "DAMPED",
                    FontFamily = new FontFamily("Consolas"),
                    FontSize = 10,
                    Foreground = new SolidColorBrush(Colors.Orange)
                };
                Grid.SetColumn(dampedBadge, 3);
                row.Children.Add(dampedBadge);
            }

            // Latest badge
            if (isNewest)
            {
                var latestText = new TextBlock
                {
                    Text = "latest",
                    FontFamily = new FontFamily("Consolas"),
                    FontSize = 10,
                    Foreground = new SolidColorBrush(Windows.UI.Color.FromArgb(0x99, 0xFF, 0xFF, 0xFF))
                };
                Grid.SetColumn(latestText, 4);
                row.Children.Add(latestText);
            }

            items.Children.Add(row);
        }

        CalibrationHistoryList.ItemsSource = null;
        CalibrationHistoryList.Items.Clear();
        CalibrationHistoryList.Items.Add(items);
    }

    private void UpdateNowPlayingSection()
    {
        var track = _appState.NowPlaying;
        if (track == null)
        {
            NowPlayingSection.Visibility = Visibility.Collapsed;
            return;
        }

        NowPlayingSection.Visibility = Visibility.Visible;
        TrackValue.Text = track.name;
        ArtistValue.Text = track.artist;
        SongIdValue.Text = track.songId;
        PositionValue.Text = $"{track.positionMs}ms / {track.durationMs}ms";
        PlayingValue.Text = _appState.IsPlaying ? "Yes" : "No";
    }

    private static string TruncatePeerId(string peerId)
    {
        if (peerId.Length <= 15)
            return peerId;
        return peerId.Substring(0, 12) + "...";
    }

    private static string FormatDrift(long drift)
    {
        return drift >= 0 ? $"+{drift}ms" : $"{drift}ms";
    }

    private static Windows.UI.Color GetDriftColor(long drift)
    {
        var absDrift = Math.Abs(drift);
        if (absDrift < 200)
            return Colors.LimeGreen;
        else if (absDrift < 1000)
            return Colors.Orange;
        else
            return Colors.Red;
    }

    private static (string text, Windows.UI.Color color) GetQuality(long drift)
    {
        var absDrift = Math.Abs(drift);
        if (absDrift < 200)
            return ("Excellent", Colors.LimeGreen);
        else if (absDrift < 500)
            return ("Good", Colors.LimeGreen);
        else if (absDrift < 1000)
            return ("Fair", Colors.Orange);
        else if (absDrift < 3000)
            return ("Poor", Colors.Orange);
        else
            return ("Bad", Colors.Red);
    }
}
