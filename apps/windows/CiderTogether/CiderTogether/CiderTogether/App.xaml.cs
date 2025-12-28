using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using CiderTogether.Models;
using CiderTogether.Views;
using H.NotifyIcon;

namespace CiderTogether;

/// <summary>
/// Provides application-specific behavior to supplement the default Application class.
/// </summary>
public partial class App : Application
{
    private MainWindow? _window;
    private TaskbarIcon? _taskbarIcon;

    /// <summary>
    /// Shared application state across all windows.
    /// </summary>
    public static AppState AppState { get; } = new();

    /// <summary>
    /// The main window instance.
    /// </summary>
    public static MainWindow? MainWindow { get; private set; }

    /// <summary>
    /// Initializes the singleton application object.
    /// </summary>
    public App()
    {
        this.InitializeComponent();
    }

    /// <summary>
    /// Invoked when the application is launched.
    /// </summary>
    protected override void OnLaunched(LaunchActivatedEventArgs args)
    {
        _window = new MainWindow();
        MainWindow = _window;
        _window.Activate();

        // Create system tray icon after window is ready
        _window.DispatcherQueue.TryEnqueue(() =>
        {
            try
            {
                CreateTaskbarIcon();
            }
            catch (Exception ex)
            {
                System.Diagnostics.Debug.WriteLine($"Failed to create taskbar icon: {ex.Message}");
            }
        });
    }

    private void CreateTaskbarIcon()
    {
        // Use ms-appx URI for packaged app resources
        _taskbarIcon = new TaskbarIcon
        {
            ToolTipText = "Cider Together",
            IconSource = new Microsoft.UI.Xaml.Media.Imaging.BitmapImage(new Uri("ms-appx:///Assets/TrayIcon.ico"))
        };

        // Force create the tray icon
        _taskbarIcon.ForceCreate();

        // Create context menu
        var contextMenu = new MenuFlyout();

        // Now Playing header (will be updated dynamically)
        var nowPlayingItem = new MenuFlyoutItem
        {
            Text = "Not Playing",
            IsEnabled = false
        };
        contextMenu.Items.Add(nowPlayingItem);
        contextMenu.Items.Add(new MenuFlyoutSeparator());

        // Open Window
        var openItem = new MenuFlyoutItem
        {
            Text = "Open Window",
            Icon = new FontIcon { Glyph = "\uE8A7" }
        };
        openItem.Click += (s, e) => _window?.DispatcherQueue.TryEnqueue(() => ShowWindow());
        contextMenu.Items.Add(openItem);

        // Leave Room (only shown when in room)
        var leaveRoomItem = new MenuFlyoutItem
        {
            Text = "Leave Room",
            Icon = new FontIcon { Glyph = "\uE7E8" }
        };
        leaveRoomItem.Click += (s, e) => _window?.DispatcherQueue.TryEnqueue(() =>
        {
            AppState.LeaveRoom();
            ShowWindow();
        });
        contextMenu.Items.Add(leaveRoomItem);

        contextMenu.Items.Add(new MenuFlyoutSeparator());

        // About
        var aboutItem = new MenuFlyoutItem
        {
            Text = "About Cider Together",
            Icon = new FontIcon { Glyph = "\uE946" }
        };
        aboutItem.Click += (s, e) => _window?.DispatcherQueue.TryEnqueue(() => ShowAboutWindow());
        contextMenu.Items.Add(aboutItem);

        // Acknowledgments
        var acknowledgementsItem = new MenuFlyoutItem
        {
            Text = "Acknowledgments",
            Icon = new FontIcon { Glyph = "\uE8D4" }
        };
        acknowledgementsItem.Click += (s, e) => _window?.DispatcherQueue.TryEnqueue(() => ShowAcknowledgmentsWindow());
        contextMenu.Items.Add(acknowledgementsItem);

        // Help (GitHub)
        var helpItem = new MenuFlyoutItem
        {
            Text = "Help (GitHub)",
            Icon = new FontIcon { Glyph = "\uE897" }
        };
        helpItem.Click += (s, e) =>
        {
            _ = Windows.System.Launcher.LaunchUriAsync(new Uri("https://github.com/giorgiobrullo/CiderTogether"));
        };
        contextMenu.Items.Add(helpItem);

        contextMenu.Items.Add(new MenuFlyoutSeparator());

        // Debug
        var debugItem = new MenuFlyoutItem
        {
            Text = "Debug",
            Icon = new FontIcon { Glyph = "\uEBE8" }
        };
        debugItem.Click += (s, e) => _window?.DispatcherQueue.TryEnqueue(() => ShowDebugWindow());
        contextMenu.Items.Add(debugItem);

        contextMenu.Items.Add(new MenuFlyoutSeparator());

        // Quit
        var quitItem = new MenuFlyoutItem
        {
            Text = "Quit",
            Icon = new FontIcon { Glyph = "\uE8BB" }
        };
        quitItem.Click += (s, e) => _window?.DispatcherQueue.TryEnqueue(() =>
        {
            AppState.LeaveRoom();
            _taskbarIcon?.Dispose();
            _window?.Close();
            Exit();
        });
        contextMenu.Items.Add(quitItem);

        _taskbarIcon.ContextFlyout = contextMenu;

        // Double-click to open window
        _taskbarIcon.DoubleClickCommand = new CommunityToolkit.Mvvm.Input.RelayCommand(ShowWindow);

        // Subscribe to state changes to update menu
        AppState.PropertyChanged += (s, e) =>
        {
            if (_window?.DispatcherQueue == null) return;

            _window.DispatcherQueue.TryEnqueue(() =>
            {
                // Update now playing text
                if (e.PropertyName == nameof(AppState.NowPlaying) || e.PropertyName == nameof(AppState.IsPlaying))
                {
                    var track = AppState.NowPlaying;
                    if (track != null)
                    {
                        var playingStatus = AppState.IsPlaying ? "▶" : "⏸";
                        nowPlayingItem.Text = $"{playingStatus} {track.name} - {track.artist}";
                    }
                    else
                    {
                        nowPlayingItem.Text = "Not Playing";
                    }
                }

                // Show/hide Leave Room based on room state
                if (e.PropertyName == nameof(AppState.IsInRoom))
                {
                    leaveRoomItem.Visibility = AppState.IsInRoom ? Visibility.Visible : Visibility.Collapsed;
                }
            });
        };

        // Initial state
        leaveRoomItem.Visibility = AppState.IsInRoom ? Visibility.Visible : Visibility.Collapsed;
    }

    public void ShowWindow()
    {
        _window?.Activate();
        if (_window != null)
        {
            _window.ShowWindow();
        }
    }

    public void HideWindow()
    {
        _window?.HideWindow();
    }

    private static AboutWindow? _aboutWindow;
    private static AcknowledgmentsWindow? _acknowledgementsWindow;
    private static DebugWindow? _debugWindow;

    public static void ShowAboutWindow()
    {
        // If window already exists and is open, just activate it
        if (_aboutWindow != null)
        {
            try
            {
                _aboutWindow.Activate();
                return;
            }
            catch
            {
                // Window was closed, create a new one
                _aboutWindow = null;
            }
        }

        _aboutWindow = new AboutWindow();
        _aboutWindow.Activate();
    }

    public static void ShowAcknowledgmentsWindow()
    {
        // If window already exists and is open, just activate it
        if (_acknowledgementsWindow != null)
        {
            try
            {
                _acknowledgementsWindow.Activate();
                return;
            }
            catch
            {
                // Window was closed, create a new one
                _acknowledgementsWindow = null;
            }
        }

        _acknowledgementsWindow = new AcknowledgmentsWindow();
        _acknowledgementsWindow.Activate();
    }

    public static void ShowDebugWindow()
    {
        // If window already exists and is open, just activate it
        if (_debugWindow != null)
        {
            try
            {
                _debugWindow.Activate();
                return;
            }
            catch
            {
                // Window was closed, create a new one
                _debugWindow = null;
            }
        }

        _debugWindow = new DebugWindow();
        _debugWindow.Activate();
    }
}
