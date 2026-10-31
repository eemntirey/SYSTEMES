using System;
using System.IO;
using System.Text;
using Newtonsoft.Json;

namespace Systemes.Native.Core
{
    public class NativeServer
    {
        public static void Start()
        {
            Console.WriteLine("SYSTEMES Native Server started. Ready for JSON-RPC over stdin/stdout.");
            while (true)
            {
                try
                {
                    string? line = Console.ReadLine();
                    if (line == null) break;
                    if (string.IsNullOrWhiteSpace(line)) continue;

                    var request = JsonConvert.DeserializeObject<dynamic>(line);
                    if (request == null)
                    {
                        Console.WriteLine(JsonConvert.SerializeObject(new { error = "Invalid JSON" }));
                        continue;
                    }

                    string method = request.method?.ToString() ?? "";
                    object result;

                    switch (method)
                    {
                        case "hardware/read":
                            result = HardwareManager.ReadRealHardware();
                            break;
                        case "processes/read":
                            result = ProcessManager.ReadRealProcesses();
                            break;
                        case "processes/kill":
                            int pid = Convert.ToInt32(request.pid);
                            result = ProcessManager.KillProcess(pid);
                            break;
                        case "files/read":
                            result = FileSystemNative.ReadRealFile(request.path?.ToString() ?? "");
                            break;
                        case "files/create":
                            result = FileSystemNative.CreateFile(request.path?.ToString() ?? "", request.content?.ToString() ?? "");
                            break;
                        case "files/delete":
                            result = FileSystemNative.DeleteFile(request.path?.ToString() ?? "");
                            break;
                        case "devices/read":
                            result = DeviceManager.ReadRealDevices();
                            break;
                        case "update/check":
                            result = WindowsUpdateNative.CheckRealUpdates();
                            break;
                        default:
                            result = new { error = "Unknown method", method };
                    }

                    string response = JsonConvert.SerializeObject(new { result, id = request.id?.ToString() ?? "" });
                    Console.WriteLine(response);
                }
                catch (Exception ex)
                {
                    Console.WriteLine(JsonConvert.SerializeObject(new { error = ex.Message }));
                }
            }
        }
    }
}
