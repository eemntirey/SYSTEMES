using System;
using System.Diagnostics;
using Newtonsoft.Json;

namespace Systemes.Native.Core
{
    public static class ProcessPhase4
    {
        public static object ReadRealProcesses()
        {
            try
            {
                var processes = Process.GetProcesses();
                var list = new System.Collections.Generic.List<object>();
                foreach (var p in processes)
                {
                    try
                    {
                        list.Add(new { pid = p.Id, name = p.ProcessName, cpu = p.TotalProcessorTime.TotalMilliseconds, memoryMB = p.WorkingSet64 / (1024 * 1024) });
                    }
                    catch { /* Process may exit during iteration */ }
                }
                return new { count = list.Count, processes = list.ToArray() };
            }
            catch { }
            return new { count = 0, processes = new object[0], note = "Données réelles non disponibles" };
        }
    }
}
