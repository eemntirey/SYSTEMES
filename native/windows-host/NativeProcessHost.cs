using System.Diagnostics;
using System.Text.Json;

namespace SYSTEMES.WindowsHost;

internal sealed class NativeProcessHost : IAsyncDisposable
{
    private readonly SemaphoreSlim _requestLock = new(1, 1);
    private Process? _process;

    public async Task StartAsync()
    {
        if (_process is { HasExited: false }) return;

        var executable = Path.Combine(AppContext.BaseDirectory, "native-engine", "SystemesNative.exe");
        if (!File.Exists(executable))
            throw new FileNotFoundException("Le moteur natif SYSTEMES est absent du dossier d'installation.", executable);

        var startInfo = new ProcessStartInfo(executable)
        {
            WorkingDirectory = Path.GetDirectoryName(executable)!,
            UseShellExecute = false,
            CreateNoWindow = true,
            RedirectStandardInput = true,
            RedirectStandardOutput = true,
            RedirectStandardError = true
        };

        _process = Process.Start(startInfo) ?? throw new InvalidOperationException("Impossible de démarrer SYSTEMES Native Core.");
        _ = DrainErrorsAsync(_process.StandardError);
        await Task.Yield();
        if (_process.HasExited)
            throw new InvalidOperationException($"SYSTEMES Native Core s'est arrêté (code {_process.ExitCode}).");
    }

    public async Task<string> SendAsync(string requestJson, CancellationToken cancellationToken = default)
    {
        await _requestLock.WaitAsync(cancellationToken);
        try
        {
            if (_process is not { HasExited: false })
                throw new InvalidOperationException("Le moteur natif n'est pas démarré.");

            await _process.StandardInput.WriteLineAsync(requestJson);
            await _process.StandardInput.FlushAsync(cancellationToken);
            var response = await _process.StandardOutput.ReadLineAsync(cancellationToken);
            if (response is null)
                throw new EndOfStreamException("Le moteur natif a fermé son canal de communication.");
            using var document = JsonDocument.Parse(response);
            return document.RootElement.GetRawText();
        }
        finally
        {
            _requestLock.Release();
        }
    }

    private static async Task DrainErrorsAsync(StreamReader reader)
    {
        while (await reader.ReadLineAsync() is not null) { }
    }

    public async ValueTask DisposeAsync()
    {
        if (_process is null) return;
        try
        {
            if (!_process.HasExited)
            {
                _process.StandardInput.Close();
                using var timeout = new CancellationTokenSource(TimeSpan.FromSeconds(3));
                await _process.WaitForExitAsync(timeout.Token);
            }
        }
        catch
        {
            try { if (!_process.HasExited) _process.Kill(entireProcessTree: true); } catch { }
        }
        _process.Dispose();
        _process = null;
        _requestLock.Dispose();
    }
}
