using System.Text.Json;
using System.Windows;
using Microsoft.Web.WebView2.Core;

namespace SYSTEMES.WindowsHost;

public partial class MainWindow : Window
{
    private readonly NativeProcessHost _native = new();

    public MainWindow()
    {
        InitializeComponent();
        Loaded += OnLoaded;
        Closed += OnClosed;
    }

    private async void OnLoaded(object sender, RoutedEventArgs e)
    {
        try
        {
            await _native.StartAsync();
            await DesktopView.EnsureCoreWebView2Async();
            DesktopView.CoreWebView2.Settings.AreDefaultContextMenusEnabled = false;
            DesktopView.CoreWebView2.Settings.AreDevToolsEnabled = false;
            DesktopView.CoreWebView2.WebMessageReceived += OnWebMessageReceived;

            var webRoot = Path.Combine(AppContext.BaseDirectory, "wwwroot");
            var entryPoint = Path.Combine(webRoot, "index.html");
            if (!File.Exists(entryPoint))
                throw new FileNotFoundException("Les fichiers de l'interface SYSTEMES sont absents. Exécutez npm run build avant la compilation Windows.", entryPoint);

            DesktopView.CoreWebView2.SetVirtualHostNameToFolderMapping(
                "systemes.local", webRoot, CoreWebView2HostResourceAccessKind.DenyCors);
            DesktopView.Source = new Uri("https://systemes.local/index.html");
        }
        catch (Exception ex)
        {
            MessageBox.Show(this, ex.Message, "SYSTEMES — démarrage impossible",
                MessageBoxButton.OK, MessageBoxImage.Error);
            Close();
        }
    }

    private async void OnWebMessageReceived(object? sender, CoreWebView2WebMessageReceivedEventArgs e)
    {
        try
        {
            using var message = JsonDocument.Parse(e.WebMessageAsJson);
            var response = await _native.SendAsync(message.RootElement.GetRawText());
            DesktopView.CoreWebView2.PostWebMessageAsJson(response);
        }
        catch (Exception ex)
        {
            var error = JsonSerializer.Serialize(new { error = ex.Message });
            DesktopView.CoreWebView2.PostWebMessageAsJson(error);
        }
    }

    private async void OnClosed(object? sender, EventArgs e)
    {
        if (DesktopView.CoreWebView2 is { } coreWebView)
            coreWebView.WebMessageReceived -= OnWebMessageReceived;
        await _native.DisposeAsync();
    }
}
