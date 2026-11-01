using System;
using System.Management;
using System.Diagnostics;

namespace Systemes.Native.Core
{
    public class ProcessManager
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
                        list.Add(new
                        {
                            id = p.Id,
                            name = p.ProcessName,
                            memoryMB = Math.Round(p.WorkingSet64 / (1024.0 * 1024.0), 2),
                            cpuUsagePercent = (double?)null // Nécessite PerformanceCounter (non implémenté ici)
                        });
                    }
                    catch { /* accès refusé pour certains processus */ }
                }
                return new { processes = list, count = list.Count, note = "Liste réelle des processus Windows" };
            }
            catch (Exception ex)
            {
                return new { error = ex.Message, note = "Aucun processus simulé" };
            }
        }

        public static object KillProcess(int pid)
        {
            try
            {
                var p = Process.GetProcessById(pid);
                p.Kill();
                return new { killed = true, pid, note = "Processus arrêté sur demande réelle" };
            }
            catch (Exception ex)
            {
                return new { killed = false, pid, error = ex.Message, note = "Arrêt non autorisé ou impossible" };
            }
        }
    }
}
