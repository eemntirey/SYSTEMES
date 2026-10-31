namespace SYSTEMES.Native;

public interface ISystemServices
{
    WindowsInfo GetWindowsInfo();
    HardwareInfo GetHardwareInfo();
    DeviceStatus[] GetDevices();
    ProcessInfo[] GetProcesses();
    bool IsUpdateAvailable();
    void RequestPrivilege(string operation, string reason);
}

public record WindowsInfo(string Version, string Build, bool IsWindows11, string Edition);
public record HardwareInfo(string CpuName, long RamUsedBytes, long RamTotalBytes, string GpuName, long DiskFreeBytes);
public record DeviceStatus(string Name, string Status, string DriverVersion);
public record ProcessInfo(string Name, int Pid, double CpuPercent, long WorkingSetBytes);
