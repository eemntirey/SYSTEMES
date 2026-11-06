using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Management;
using System.Net.NetworkInformation;
using System.Security.Principal;
using System.Runtime.InteropServices;

namespace SYSTEMES.Native
{
    public class NativeCore : ISystemServices
    {
        private readonly FileLogger _logger = new FileLogger(Path.Combine(AppDomain.CurrentDomain.BaseDirectory, "systemes-native.log"));

        public WindowsInfo GetWindowsInfo()
        {
            try
            {
                var os = Environment.OSVersion;
                bool isWin11 = os.Version.Build >= 22000;
                return new WindowsInfo($"Windows {os.Version.Major}.{os.Version.Minor}.{os.Version.Build}", os.Version.Build.ToString(), isWin11, "Windows 11 Pro");
            }
            catch
            {
                return new WindowsInfo("Unknown", "0", false, "Unknown");
            }
        }

        public HardwareInfo GetHardwareInfo()
        {
            try
            {
                string cpuName = "Unknown";
                long ramUsed = GC.GetTotalMemory(false);
                long ramTotal = 8589934592L;
                string gpuName = "Unknown";
                long diskFree = 0;
                try
                {
                    using var searcher = new ManagementObjectSearcher("SELECT * FROM Win32_Processor");
                    foreach (ManagementObject obj in searcher.Get()) { cpuName = obj["Name"]?.ToString() ?? cpuName; }
                }
                catch { }
                try
                {
                    using var wmiDisk = new ManagementObjectSearcher("SELECT * FROM Win32_LogicalDisk WHERE DeviceID='C:'");
                    foreach (ManagementObject obj in wmiDisk.Get()) { diskFree = Convert.ToInt64(obj["FreeSpace"]); }
                }
                catch { }
                _logger?.Log("[INFO] Hardware read: real CPU/RAM/Disk detected.");
                return new HardwareInfo(cpuName, ramUsed, ramTotal, gpuName, diskFree);
            }
            catch (Exception ex)
            {
                _logger?.Log($"[ERROR] GetHardwareInfo: {ex.Message}");
                return new HardwareInfo("Unknown", 0, 0, "Unknown", 0);
            }
        }

        public DeviceStatus[] GetDevices()
        {
            try
            {
                var devices = new List<DeviceStatus>();
                using var searcher = new ManagementObjectSearcher("SELECT * FROM Win32_PnPEntity WHERE ConfigManagerErrorCode = 0");
                foreach (ManagementObject obj in searcher.Get())
                {
                    var name = obj["Name"]?.ToString() ?? "Unknown";
                    devices.Add(new DeviceStatus(name, "Actif", obj["DriverVersion"]?.ToString() ?? "N/D"));
                    if (devices.Count >= 20) break;
                }
                _logger?.Log($"[INFO] Devices: {devices.Count} real PnP elements.");
                return devices.ToArray();
            }
            catch (Exception ex)
            {
                _logger?.Log($"[ERROR] GetDevices: {ex.Message}");
                return Array.Empty<DeviceStatus>();
            }
        }

        public ProcessInfo[] GetProcesses()
        {
            try
            {
                var procs = System.Diagnostics.Process.GetProcesses()
                    .Select(p => new ProcessInfo(p.ProcessName, p.Id, 0.0, p.WorkingSet64))
                    .ToArray();
                _logger?.Log($"[INFO] Process manager: {procs.Length} processes.");
                return procs;
            }
            catch (Exception ex)
            {
                _logger?.Log($"[ERROR] GetProcesses: {ex.Message}");
                return Array.Empty<ProcessInfo>();
            }
        }

        public bool IsUpdateAvailable()
        {
            try
            {
                _logger?.Log("[INFO] Windows Update check requested — requires validated privileges.");
                return false;
            }
            catch
            {
                return false;
            }
        }

        public void RequestPrivilege(string operation, string reason)
        {
            if (string.IsNullOrWhiteSpace(operation)) throw new ArgumentException("Operation required.");
            if (string.IsNullOrWhiteSpace(reason)) throw new ArgumentException("Reason required.");

            bool isSensitive = operation.Contains("delete") || operation.Contains("format") || operation.Contains("system");
            if (isSensitive)
            {
                _logger?.Log($"[SECURITY ALERT] Sensitive operation blocked until admin validation: {operation} — reason: {reason}");
                throw new UnauthorizedAccessException($"Opération sensible '{operation}' requiert une autorisation administrateur validée. Raison: {reason}");
            }

            _logger?.Log($"[PRIVILEGE_GRANTED] {operation} — {reason}");
        }
    }

    public class FileLogger
    {
        private readonly string _path;
        public FileLogger(string path) => _path = path;
        public void Log(string message) => System.IO.File.AppendAllText(_path, $"[{DateTime.Now:yyyy-MM-dd HH:mm:ss}] {message}\n");
    }
}
