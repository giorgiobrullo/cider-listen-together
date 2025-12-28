using System.Runtime.InteropServices.WindowsRuntime;
using Microsoft.Graphics.Canvas;
using Microsoft.Graphics.Canvas.Effects;
using Microsoft.Graphics.Canvas.UI.Xaml;
using Microsoft.UI;
using Microsoft.UI.Composition.SystemBackdrops;
using Microsoft.UI.Windowing;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Media;
using CiderTogether.Models;
using CiderTogether.Views;
using Windows.Graphics;
using WinRT.Interop;

namespace CiderTogether;

/// <summary>
/// Main application window with blurred artwork backdrop and custom title bar.
/// </summary>
public sealed partial class MainWindow : Window
{
    private readonly AppState _appState;
    private AppWindow? _appWindow;
    private CanvasBitmap? _artworkBitmap;
    private string? _currentArtworkUrl;
    private double _scaleFactor = 1.0;

    // Window size constraints (in logical pixels, like macOS)
    private const int MinWidth = 400;
    private const int MinHeight = 550;
    private const int DefaultWidth = 420;
    private const int DefaultHeight = 650;

    public MainWindow()
    {
        this.InitializeComponent();

        _appState = App.AppState;

        // Configure window
        ConfigureWindow();
        ConfigureTitleBar();
        ConfigureBackdrop();

        // Subscribe to state changes
        _appState.PropertyChanged += AppState_PropertyChanged;

        // Navigate to initial view
        NavigateToCurrentView();

        // Start connection check (fire and forget, errors are handled internally)
        _ = Task.Run(async () =>
        {
            try
            {
                await _appState.OnAppearAsync();
            }
            catch
            {
                // Errors are handled in OnAppearAsync
            }
        });
    }

    private void ConfigureWindow()
    {
        // Get the AppWindow for this Window
        var hwnd = WindowNative.GetWindowHandle(this);
        var windowId = Win32Interop.GetWindowIdFromWindow(hwnd);
        _appWindow = AppWindow.GetFromWindowId(windowId);

        // Set window icon for taskbar
        _appWindow.SetIcon("Assets/TrayIcon.ico");

        // Get DPI scale factor for proper sizing
        var dpi = GetDpiForWindow(hwnd);
        _scaleFactor = dpi / 96.0;

        // Set initial window size (scaled for DPI)
        var scaledWidth = (int)(DefaultWidth * _scaleFactor);
        var scaledHeight = (int)(DefaultHeight * _scaleFactor);
        _appWindow.Resize(new SizeInt32(scaledWidth, scaledHeight));

        // Subscribe to size changes to enforce constraints
        _appWindow.Changed += AppWindow_Changed;

        // Force dark theme
        if (Content is FrameworkElement root)
        {
            root.RequestedTheme = ElementTheme.Dark;
        }
    }

    [System.Runtime.InteropServices.DllImport("user32.dll")]
    private static extern uint GetDpiForWindow(IntPtr hwnd);

    private void AppWindow_Changed(AppWindow sender, AppWindowChangedEventArgs args)
    {
        if (!args.DidSizeChange)
            return;

        var currentSize = sender.Size;
        var newWidth = currentSize.Width;
        var newHeight = currentSize.Height;
        var needsResize = false;

        // Enforce min constraints (scaled for DPI)
        var scaledMinWidth = (int)(MinWidth * _scaleFactor);
        var scaledMinHeight = (int)(MinHeight * _scaleFactor);

        if (newWidth < scaledMinWidth) { newWidth = scaledMinWidth; needsResize = true; }
        if (newHeight < scaledMinHeight) { newHeight = scaledMinHeight; needsResize = true; }

        if (needsResize)
        {
            sender.Resize(new SizeInt32(newWidth, newHeight));
        }

        // Redraw blur canvas on resize
        BlurCanvas.Invalidate();
    }

    private void ConfigureTitleBar()
    {
        // Extend content into title bar
        ExtendsContentIntoTitleBar = true;
        SetTitleBar(AppTitleBar);

        // Hide default title
        if (_appWindow != null)
        {
            _appWindow.Title = "Cider Together";
        }
    }

    private void ConfigureBackdrop()
    {
        // Try to use Mica backdrop
        if (MicaController.IsSupported())
        {
            SystemBackdrop = new MicaBackdrop { Kind = MicaKind.Base };
        }
        else if (DesktopAcrylicController.IsSupported())
        {
            // Fall back to Acrylic if Mica isn't supported
            SystemBackdrop = new DesktopAcrylicBackdrop();
        }
    }

    private void AppState_PropertyChanged(object? sender, System.ComponentModel.PropertyChangedEventArgs e)
    {
        // Handle property changes on UI thread
        DispatcherQueue.TryEnqueue(() =>
        {
            switch (e.PropertyName)
            {
                case nameof(AppState.ViewState):
                    NavigateToCurrentView();
                    break;
                case nameof(AppState.CiderConnected):
                    UpdateConnectionStatus();
                    break;
                case nameof(AppState.IsInRoom):
                    UpdateRoomActions();
                    _ = UpdateBackgroundArtworkAsync();
                    break;
                case nameof(AppState.NowPlaying):
                case nameof(AppState.RoomState):
                    _ = UpdateBackgroundArtworkAsync();
                    break;
            }
        });
    }

    private void NavigateToCurrentView()
    {
        switch (_appState.ViewState)
        {
            case ViewState.Home:
                ContentFrame.Navigate(typeof(HomeView));
                break;
            case ViewState.Creating:
            case ViewState.Joining:
                ContentFrame.Navigate(typeof(JoiningView));
                break;
            case ViewState.InRoom:
                ContentFrame.Navigate(typeof(RoomView));
                break;
        }
    }

    private void UpdateConnectionStatus()
    {
        ConnectionStatus.Visibility = _appState.CiderConnected
            ? Visibility.Visible
            : Visibility.Collapsed;
    }

    private void UpdateRoomActions()
    {
        RoomActions.Visibility = _appState.IsInRoom
            ? Visibility.Visible
            : Visibility.Collapsed;
    }

    private async Task UpdateBackgroundArtworkAsync()
    {
        // Use room state's track for listeners, local for host or when not in room
        var displayTrack = (_appState.IsInRoom && !_appState.IsHost)
            ? _appState.RoomState?.currentTrack
            : _appState.NowPlaying;
        var artworkUrl = displayTrack?.artworkUrl;

        // Skip if same URL
        if (artworkUrl == _currentArtworkUrl)
            return;

        _currentArtworkUrl = artworkUrl;

        if (string.IsNullOrEmpty(artworkUrl))
        {
            _artworkBitmap = null;
            BlurCanvas.Visibility = Visibility.Collapsed;
            BackgroundOverlay.Visibility = Visibility.Collapsed;
            return;
        }

        try
        {
            // Load the artwork bitmap
            using var httpClient = new System.Net.Http.HttpClient();
            var imageBytes = await httpClient.GetByteArrayAsync(artworkUrl);

            using var stream = new MemoryStream(imageBytes);
            using var randomAccessStream = stream.AsRandomAccessStream();

            _artworkBitmap = await CanvasBitmap.LoadAsync(BlurCanvas, randomAccessStream);

            BlurCanvas.Visibility = Visibility.Visible;
            BackgroundOverlay.Visibility = Visibility.Visible;
            BlurCanvas.Invalidate();
        }
        catch
        {
            _artworkBitmap = null;
            BlurCanvas.Visibility = Visibility.Collapsed;
            BackgroundOverlay.Visibility = Visibility.Collapsed;
        }
    }

    private void BlurCanvas_Draw(CanvasControl sender, CanvasDrawEventArgs args)
    {
        if (_artworkBitmap == null)
            return;

        var session = args.DrawingSession;
        var canvasWidth = (float)sender.ActualWidth;
        var canvasHeight = (float)sender.ActualHeight;

        if (canvasWidth <= 0 || canvasHeight <= 0)
            return;

        // Calculate scale to cover the entire canvas (like UniformToFill)
        var bitmapWidth = (float)_artworkBitmap.SizeInPixels.Width;
        var bitmapHeight = (float)_artworkBitmap.SizeInPixels.Height;

        var scaleX = canvasWidth / bitmapWidth;
        var scaleY = canvasHeight / bitmapHeight;
        var scale = Math.Max(scaleX, scaleY);

        var scaledWidth = bitmapWidth * scale;
        var scaledHeight = bitmapHeight * scale;

        // Center the image
        var offsetX = (canvasWidth - scaledWidth) / 2;
        var offsetY = (canvasHeight - scaledHeight) / 2;

        // Create scale effect
        var scaleEffect = new ScaleEffect
        {
            Source = _artworkBitmap,
            Scale = new System.Numerics.Vector2(scale, scale),
            CenterPoint = new System.Numerics.Vector2(0, 0)
        };

        // Apply heavy Gaussian blur (radius 60 like macOS)
        var blurEffect = new GaussianBlurEffect
        {
            Source = scaleEffect,
            BlurAmount = 60f,
            BorderMode = EffectBorderMode.Hard
        };

        // Draw the blurred image
        session.DrawImage(blurEffect, offsetX, offsetY);
    }

    private void MinimizeToTray_Click(object sender, RoutedEventArgs e)
    {
        HideWindow();
    }

    /// <summary>
    /// Hides the window to system tray.
    /// </summary>
    public void HideWindow()
    {
        if (_appWindow?.Presenter is OverlappedPresenter presenter)
        {
            presenter.Minimize();
        }
        // Hide the window from taskbar (minimize to tray)
        var hwnd = WindowNative.GetWindowHandle(this);
        ShowWindow(hwnd, SW_HIDE);
    }

    /// <summary>
    /// Shows the window from system tray.
    /// </summary>
    public void ShowWindow()
    {
        var hwnd = WindowNative.GetWindowHandle(this);
        ShowWindow(hwnd, SW_SHOW);
        if (_appWindow?.Presenter is OverlappedPresenter presenter)
        {
            presenter.Restore();
        }
        Activate();
    }

    private const int SW_HIDE = 0;
    private const int SW_SHOW = 5;

    [System.Runtime.InteropServices.DllImport("user32.dll")]
    private static extern bool ShowWindow(IntPtr hWnd, int nCmdShow);

    private void LeaveRoom_Click(object sender, RoutedEventArgs e)
    {
        _appState.LeaveRoom();
    }

    private void About_Click(object sender, RoutedEventArgs e)
    {
        App.ShowAboutWindow();
    }

    private void Acknowledgments_Click(object sender, RoutedEventArgs e)
    {
        App.ShowAcknowledgmentsWindow();
    }

    private void Help_Click(object sender, RoutedEventArgs e)
    {
        _ = Windows.System.Launcher.LaunchUriAsync(new Uri("https://github.com/giorgiobrullo/cider-listen-together"));
    }

    private void Debug_Click(object sender, RoutedEventArgs e)
    {
        App.ShowDebugWindow();
    }

    private void Quit_Click(object sender, RoutedEventArgs e)
    {
        _appState.LeaveRoom();
        Close();
        Application.Current.Exit();
    }
}
