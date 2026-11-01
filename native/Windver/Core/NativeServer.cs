using System;
using Newtonsoft.Json;
using Newtonsoft.Json.Linq;

namespace Systemes.Native.Core
{
    public class NativeServer
    {
        public static void Start()
        {
            Console.Error.WriteLine("SYSTEMES Native Server ready.");
            while (true)
            {
                string? line = Console.ReadLine();
                if (line == null) break;
                if (string.IsNullOrWhiteSpace(line)) continue;

                JToken? id = null;
                try
                {
                    var request = JObject.Parse(line);
                    id = request["id"]?.DeepClone();
                    var args = request["params"] as JObject ?? request;
                    var method = request["method"]?.Value<string>() ?? "";
                    object result;

                    switch (method)
                    {
                        case "system/ping":
                            result = new { status = "ready", engine = "SYSTEMES Native Core" };
                            break;
                        case "hardware/read":
                            result = HardwareManager.ReadRealHardware();
                            break;
                        case "processes/read":
                            result = ProcessManager.ReadRealProcesses();
                            break;
                        case "processes/kill":
                            result = ProcessManager.KillProcess(args["pid"]?.Value<int>() ?? 0);
                            break;
                        case "files/read":
                            result = FileSystemNative.ReadRealFile(args["path"]?.Value<string>() ?? "");
                            break;
                        case "files/create":
                            result = FileSystemNative.CreateFile(args["path"]?.Value<string>() ?? "", args["content"]?.Value<string>() ?? "");
                            break;
                        case "files/delete":
                            result = FileSystemNative.DeleteFile(args["path"]?.Value<string>() ?? "");
                            break;
                        case "devices/read":
                            result = DeviceManager.ReadRealDevices();
                            break;
                        case "update/check":
                            result = WindowsUpdateNative.CheckRealUpdates();
                            break;
                        default:
                            result = new { error = "Unknown method", method };
                            break;
                    }

                    Console.WriteLine(JsonConvert.SerializeObject(new { result, id }));
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
