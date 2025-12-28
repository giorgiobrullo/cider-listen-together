using Microsoft.UI;
using Microsoft.UI.Composition.SystemBackdrops;
using Microsoft.UI.Windowing;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Media.Imaging;
using Windows.Graphics;
using WinRT.Interop;

namespace CiderTogether.Views;

public sealed partial class AboutWindow : Window
{
    private AppWindow? _appWindow;

    // Window size in logical pixels
    private const int WindowWidth = 340;
    private const int WindowHeight = 580;

    public AboutWindow()
    {
        this.InitializeComponent();

        ConfigureWindow();
        ConfigureTitleBar();
        ConfigureBackdrop();
        LoadContent();
    }

    [System.Runtime.InteropServices.DllImport("user32.dll")]
    private static extern uint GetDpiForWindow(IntPtr hwnd);

    private void ConfigureWindow()
    {
        // Get the AppWindow
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

            _appWindow.Title = "About Cider Together";

            // Close only - no resize, maximize, or minimize
            if (_appWindow.Presenter is OverlappedPresenter presenter)
            {
                presenter.IsResizable = false;
                presenter.IsMaximizable = false;
                presenter.IsMinimizable = false;
            }
        }

        // Force dark theme
        if (Content is FrameworkElement root)
        {
            root.RequestedTheme = ElementTheme.Dark;
        }
    }

    private void ConfigureTitleBar()
    {
        // Extend content into title bar (like MainWindow)
        ExtendsContentIntoTitleBar = true;
        SetTitleBar(AppTitleBar);
    }

    private void ConfigureBackdrop()
    {
        // Use Mica backdrop like MainWindow
        if (MicaController.IsSupported())
        {
            SystemBackdrop = new MicaBackdrop { Kind = MicaKind.Base };
        }
        else if (DesktopAcrylicController.IsSupported())
        {
            SystemBackdrop = new DesktopAcrylicBackdrop();
        }
    }

    private void LoadContent()
    {
        // Set version
        var version = typeof(App).Assembly.GetName().Version;
        VersionText.Text = $"Version {version?.Major ?? 1}.{version?.Minor ?? 0}.{version?.Build ?? 0}";

        // Load app icon from assets - try highest quality first
        var iconPaths = new[]
        {
            "ms-appx:///Assets/StoreLogo.scale-400.png",
            "ms-appx:///Assets/StoreLogo.scale-200.png",
            "ms-appx:///Assets/Square150x150Logo.scale-400.png",
            "ms-appx:///Assets/Square150x150Logo.scale-200.png",
            "ms-appx:///Assets/Square44x44Logo.targetsize-256.png"
        };

        bool loaded = false;
        foreach (var path in iconPaths)
        {
            try
            {
                var iconUri = new Uri(path);
                AppIconBrush.ImageSource = new BitmapImage(iconUri);
                loaded = true;
                break;
            }
            catch
            {
                // Try next
            }
        }

        if (!loaded)
        {
            // Show fallback icon
            FallbackIcon.Visibility = Visibility.Visible;
        }
    }
}
