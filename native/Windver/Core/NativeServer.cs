using System;
using Newtonsoft.Json;
using Newtonsoft.Json.Linq;

namespace Systemes.Native.Core
{
    public class NativeServer
    {
        private const int MaxRequestCharacters = 1024 * 1024;
        private const int MaxRequestIdCharacters = 128;
        private const int MaxMethodCharacters = 128;

        public static void Start()
        {
            Console.Error.WriteLine("SYSTEMES Native Server ready.");
            while (true)
            {
                string? line = Console.ReadLine();
                if (line == null) break;
                if (string.IsNullOrWhiteSpace(line)) continue;

                if (line.Length > MaxRequestCharacters)
                {
                    Console.WriteLine(JsonConvert.SerializeObject(new { error = "La requête IPC dépasse la limite de 1 Mio.", id = (string?)null }));
                    Console.Out.Flush();
                    continue;
                }

                JToken? id = null;
                try
                {
                    var request = JObject.Parse(line);
                    var idToken = request["id"];
                    if (idToken?.Type != JTokenType.String)
                        throw new ArgumentException("L’identifiant IPC doit être une chaîne de caractères.");
                    var requestId = idToken.Value<string>();
                    if (string.IsNullOrWhiteSpace(requestId) || requestId.Length > MaxRequestIdCharacters)
                        throw new ArgumentException("L’identifiant IPC est vide ou dépasse 128 caractères.");
                    id = idToken.DeepClone();

                    var methodToken = request["method"];
                    if (methodToken?.Type != JTokenType.String)
                        throw new ArgumentException("La méthode IPC doit être une chaîne de caractères.");
                    var method = methodToken.Value<string>() ?? "";
                    if (string.IsNullOrWhiteSpace(method) || method.Length > MaxMethodCharacters)
                        throw new ArgumentException("La méthode IPC est vide ou dépasse 128 caractères.");

                    var paramsToken = request["params"];
                    if (paramsToken != null && paramsToken.Type != JTokenType.Object)
                        throw new ArgumentException("Les paramètres IPC doivent être un objet JSON.");
                    var args = paramsToken as JObject ?? new JObject();
                    object result;

                    switch (method)
                    {
                        case "system/ping":
                            result = new { status = "ready", engine = "SYSTEMES Native Core" };
                            break;
                        case "terminal/execute":
                            result = TerminalNative.Execute(
                                args["command"]?.Value<string>() ?? "",
                                args["workingDirectory"]?.Value<string>() ?? "");
                            break;
                        case "security/read":
                            result = SecurityManager.ReadSecurityContext();
                            break;
                        case "system/open-settings":
                            result = SettingsNative.OpenSettingsPage(args["page"]?.Value<string>() ?? "");
                            break;
                        case "system/power/read":
                            result = PowerNative.ReadPowerSchemes();
                            break;
                        case "system/power/set":
                            result = PowerNative.SetPowerScheme(args["guid"]?.Value<string>() ?? "");
                            break;
                        case "hardware/read":
                            result = HardwareManager.ReadRealHardware();
                            break;
                        case "memory/read":
                            result = MemoryManager.ReadRealMemory();
                            break;
                        case "processes/read":
                            result = ProcessManager.ReadRealProcesses();
                            break;
                        case "processes/kill":
                            result = ProcessManager.KillProcess(args["pid"]?.Value<int>() ?? 0, args["name"]?.Value<string>(), args["startTimeUtcTicks"]?.Value<string>());
                            break;
                        case "files/locations":
                            result = FileSystemNative.ReadKnownLocations();
                            break;
                        case "files/list":
                            result = FileSystemNative.ListDirectory(args["path"]?.Value<string>() ?? "");
                            break;
                        case "files/read":
                            result = FileSystemNative.ReadRealFile(args["path"]?.Value<string>() ?? "");
                            break;
                        case "files/read-content":
                            result = FileSystemNative.ReadTextFile(args["path"]?.Value<string>() ?? "");
                            break;
                        case "files/write":
                            result = FileSystemNative.WriteTextFile(args["path"]?.Value<string>() ?? "", args["content"]?.Value<string>() ?? "");
                            break;
                        case "files/create-directory":
                            result = FileSystemNative.CreateDirectory(args["path"]?.Value<string>() ?? "");
                            break;
                        case "files/create":
                            result = FileSystemNative.CreateFile(args["path"]?.Value<string>() ?? "", args["content"]?.Value<string>() ?? "");
                            break;
                        case "files/recycle-bin/open":
                            result = FileSystemNative.OpenRecycleBin();
                            break;
                        case "files/delete":
                            result = FileSystemNative.DeleteFile(args["path"]?.Value<string>() ?? "");
                            break;
                        case "files/permissions/read":
                            result = FileSystemNative.ReadPermissions(args["path"]?.Value<string>() ?? "");
                            break;
                        case "files/rename":
                            result = FileSystemNative.RenameEntry(
                                args["path"]?.Value<string>() ?? "",
                                args["newName"]?.Value<string>() ?? "");
                            break;
                        case "files/copy":
                            result = FileSystemNative.CopyEntry(
                                args["source"]?.Value<string>() ?? "",
                                args["destination"]?.Value<string>() ?? "");
                            break;
                        case "files/move":
                            result = FileSystemNative.MoveEntry(
                                args["source"]?.Value<string>() ?? "",
                                args["destination"]?.Value<string>() ?? "");
                            break;
                        case "devices/read":
                            result = DeviceManager.ReadRealDevices();
                            break;
                        case "services/read":
                            result = ServiceManager.ReadServices();
                            break;
                        case "services/control":
                            result = ServiceManager.ControlService(args["name"]?.Value<string>() ?? "", args["action"]?.Value<string>() ?? "");
                            break;
                        case "audio/read":
                            result = AudioManager.ReadRealAudio();
                            break;
                        case "audio/set-volume":
                            result = AudioManager.SetRealVolume(args["percent"]?.Value<int>() ?? -1);
                            break;
                        case "audio/set-mute":
                            result = AudioManager.SetRealMute(args["muted"]?.Value<bool>() ?? false);
                            break;
                        case "audio/devices":
                            result = AudioManager.ListRealAudioDevices();
                            break;
                        case "display/read":
                            result = DisplayManager.ReadRealDisplay();
                            break;
                        case "power/read":
                            result = BrightnessAndBattery.ReadRealPower();
                            break;
                        case "network/read":
                            result = NetworkManager.ReadRealNetwork();
                            break;
                        case "network/wifi/scan":
                            result = NetworkManager.ScanWifi();
                            break;
                        case "network/wifi/status":
                            result = NetworkManager.ReadWifiStatus();
                            break;
                        case "network/wifi/connect":
                            result = NetworkManager.ConnectWifi(
                                args["ssid"]?.Value<string>() ?? "",
                                args["password"]?.Value<string>());
                            break;
                        case "network/wifi/disconnect":
                            result = NetworkManager.DisconnectWifi();
                            break;
                        case "network/wifi/adapter-status":
                            result = RadioControlNative.ReadWifiEnabled();
                            break;
                        case "network/wifi/set-enabled":
                            result = RadioControlNative.SetWifiEnabled(args["enabled"]?.Value<bool>() ?? false);
                            break;
                        case "bluetooth/set-enabled":
                            result = RadioControlNative.SetBluetoothEnabled(args["enabled"]?.Value<bool>() ?? false);
                            break;
                        case "bluetooth/status":
                            result = BluetoothManager.ReadStatus();
                            break;
                        case "bluetooth/scan":
                            result = BluetoothManager.ScanDevices(args["timeoutSeconds"]?.Value<int>() ?? 6);
                            break;
                        case "bluetooth/pair":
                            result = BluetoothManager.PairDevice(args["deviceId"]?.Value<string>() ?? "");
                            break;
                        case "update/check":
                            result = WindowsUpdateNative.CheckRealUpdates();
                            break;
                        default:
                            throw new NotSupportedException($"Unknown native method: {method}");
                    }

                    var resultToken = JToken.FromObject(result);
                    if (resultToken is JObject resultObject && resultObject["error"]?.Type == JTokenType.String)
                        throw new InvalidOperationException(resultObject["error"]!.ToString());

                    Console.WriteLine(JsonConvert.SerializeObject(new { result = resultToken, id }));
                    Console.Out.Flush();
                }
                catch (Exception ex)
                {
                    Console.WriteLine(JsonConvert.SerializeObject(new { error = ex.Message, id }));
                    Console.Out.Flush();
                }
            }
        }
    }
}
