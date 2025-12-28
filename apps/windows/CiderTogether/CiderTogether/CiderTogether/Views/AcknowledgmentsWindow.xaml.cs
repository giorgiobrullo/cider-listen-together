using Microsoft.UI.Composition.SystemBackdrops;
using Microsoft.UI.Windowing;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Media;
using Windows.Graphics;
using WinRT.Interop;
using Microsoft.UI;

namespace CiderTogether.Views;

public sealed partial class AcknowledgmentsWindow : Window
{
    private AppWindow? _appWindow;

    // Window size in logical pixels
    private const int WindowWidth = 500;
    private const int WindowHeight = 550;

    public AcknowledgmentsWindow()
    {
        this.InitializeComponent();

        ConfigureWindow();
        ConfigureTitleBar();
        ConfigureBackdrop();
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

            _appWindow.Title = "Acknowledgments";
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
}
