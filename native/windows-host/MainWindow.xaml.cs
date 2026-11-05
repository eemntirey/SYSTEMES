using System.IO;
using System.Runtime.InteropServices;
using System.Text.Json;
using System.Windows;
using Microsoft.Web.WebView2.Core;

namespace SYSTEMES.WindowsHost;

public partial class MainWindow : Window
{
    private readonly NativeProcessHost _native = new();
    private bool _uiSmokeReported;

    private const int SM_XVIRTUALSCREEN = 76;
    private const int SM_YVIRTUALSCREEN = 77;
    private const int SM_CXVIRTUALSCREEN = 78;
    private const int SM_CYVIRTUALSCREEN = 79;

    [DllImport("user32.dll")]
    private static extern int GetSystemMetrics(int nIndex);

    public MainWindow()
    {
        InitializeComponent();
        Loaded += OnLoaded;
        Closed += OnClosed;
        SourceInitialized += OnSourceInitialized;
    }

    /// <summary>
    /// Étendre la fenêtre hôte sur tout le bureau virtuel Windows
    /// pour que le placement React multi-écrans corresponde aux moniteurs physiques.
    /// Désactiver via SYSTEMES_SINGLE_MONITOR=1.
    /// </summary>
    private void OnSourceInitialized(object? sender, EventArgs e)
    {
        try
        {
            if (string.Equals(Environment.GetEnvironmentVariable("SYSTEMES_SINGLE_MONITOR"), "1", StringComparison.Ordinal))
                return;

            int vx = GetSystemMetrics(SM_XVIRTUALSCREEN);
            int vy = GetSystemMetrics(SM_YVIRTUALSCREEN);
            int vw = GetSystemMetrics(SM_CXVIRTUALSCREEN);
            int vh = GetSystemMetrics(SM_CYVIRTUALSCREEN);
            if (vw < 800 || vh < 600) return;

            WindowStartupLocation = WindowStartupLocation.Manual;
            WindowState = WindowState.Normal;
            Left = vx;
            Top = vy;
            Width = vw;
            Height = vh;
        }
        catch
        {
            // Garder le layout XAML par défaut si metrics indisponibles.
        }
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
            DesktopView.CoreWebView2.NavigationCompleted += OnNavigationCompleted;

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
            ReportUiSmokeFailure(ex);
            MessageBox.Show(this, ex.Message, "SYSTEMES — démarrage impossible",
                MessageBoxButton.OK, MessageBoxImage.Error);
            Close();
        }
    }

    private async void OnNavigationCompleted(object? sender, CoreWebView2NavigationCompletedEventArgs e)
    {
        var resultPath = Environment.GetEnvironmentVariable("SYSTEMES_UI_SMOKE_RESULT");
        if (string.IsNullOrWhiteSpace(resultPath) || _uiSmokeReported)
            return;

        try
        {
            if (!e.IsSuccess)
                throw new InvalidOperationException($"WebView2 navigation failed: {e.WebErrorStatus}");

            var raw = await DesktopView.CoreWebView2.ExecuteScriptAsync(
                "JSON.stringify({readyState:document.readyState,title:document.title,url:location.href,rootChildren:document.querySelector('#root')?.childElementCount ?? 0,bodyTextLength:document.body?.innerText?.trim().length ?? 0})");
            var state = JsonDocument.Parse(JsonSerializer.Deserialize<string>(raw) ?? "{}");
            var rootChildren = state.RootElement.GetProperty("rootChildren").GetInt32();
            var bodyTextLength = state.RootElement.GetProperty("bodyTextLength").GetInt32();
            var readyState = state.RootElement.GetProperty("readyState").GetString();

            if (readyState != "complete" || rootChildren < 1 || bodyTextLength < 1)
                throw new InvalidOperationException($"React UI did not render: readyState={readyState}, rootChildren={rootChildren}, bodyTextLength={bodyTextLength}");

            Directory.CreateDirectory(Path.GetDirectoryName(resultPath)!);
            await File.WriteAllTextAsync(resultPath, JsonSerializer.Serialize(new
            {
                status = "ready",
                title = state.RootElement.GetProperty("title").GetString(),
                url = state.RootElement.GetProperty("url").GetString(),
                rootChildren,
                bodyTextLength,
                webView2 = "navigation-completed"
            }));
            _uiSmokeReported = true;
        }
        catch (Exception ex)
        {
            ReportUiSmokeFailure(ex);
        }
    }

    private static string ClassifyHostError(Exception exception)
    {
        if (exception is UnauthorizedAccessException || exception is System.Security.SecurityException)
            return "permission-required";
        if (exception is PlatformNotSupportedException || exception is NotSupportedException)
            return "unsupported";
        if (exception is TimeoutException)
            return "timeout";
        if (exception is FileNotFoundException || exception is DirectoryNotFoundException)
            return "not-found";
        if (exception is IOException)
            return "host-unavailable";
        return "native-error";
    }

    private void ReportUiSmokeFailure(Exception exception)
    {
        var resultPath = Environment.GetEnvironmentVariable("SYSTEMES_UI_SMOKE_RESULT");
        if (string.IsNullOrWhiteSpace(resultPath) || _uiSmokeReported)
            return;

        try
        {
            Directory.CreateDirectory(Path.GetDirectoryName(resultPath)!);
            File.WriteAllText(resultPath, JsonSerializer.Serialize(new { status = "failed", error = exception.ToString() }));
            _uiSmokeReported = true;
        }
        catch
        {
        }
    }

    private async void OnWebMessageReceived(object? sender, CoreWebView2WebMessageReceivedEventArgs e)
    {
        // Only the packaged SYSTEMES origin may invoke privileged native operations.
        if (!Uri.TryCreate(e.Source, UriKind.Absolute, out var source)
            || !string.Equals(source.Scheme, Uri.UriSchemeHttps, StringComparison.OrdinalIgnoreCase)
            || !string.Equals(source.Host, "systemes.local", StringComparison.OrdinalIgnoreCase))
            return;

        string? requestId = null;
        try
        {
            if (e.WebMessageAsJson.Length > 1024 * 1024)
                throw new InvalidOperationException("La requête WebView2 dépasse la limite de 1 Mio.");

            using var message = JsonDocument.Parse(e.WebMessageAsJson);
            if (message.RootElement.TryGetProperty("id", out var idElement))
                requestId = idElement.GetString();

            var response = await _native.SendAsync(message.RootElement.GetRawText());
            DesktopView.CoreWebView2.PostWebMessageAsJson(response);
        }
        catch (Exception ex)
        {
            var error = JsonSerializer.Serialize(new
            {
                id = requestId,
                error = ex.Message,
                errorCode = ClassifyHostError(ex),
                retryable = ex is TimeoutException || ex is IOException
            });
            DesktopView.CoreWebView2.PostWebMessageAsJson(error);
        }
    }

    private async void OnClosed(object? sender, EventArgs e)
    {
        if (DesktopView.CoreWebView2 is { } coreWebView)
        {
            coreWebView.WebMessageReceived -= OnWebMessageReceived;
            coreWebView.NavigationCompleted -= OnNavigationCompleted;
        }
        await _native.DisposeAsync();
    }
}
