using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;

namespace Systemes.Native.Core
{
    // ===== Core Audio — MMDevice API (COM interop, aucune dépendance externe) =====

    [ComImport, Guid("BCDE0395-E52F-467C-8E3D-C4579291692E")]
    internal class MMDeviceEnumeratorComObject { }

    [ComImport, Guid("A95664D2-9614-4F35-A746-DE8DB63617E6"),
     InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
    internal interface IMMDeviceEnumerator
    {
        [PreserveSig] int EnumAudioEndpoints(int dataFlow, int stateMask, out IMMDeviceCollection devices);
        [PreserveSig] int GetDefaultAudioEndpoint(int dataFlow, int role, out IMMDevice device);
        [PreserveSig] int GetDevice([MarshalAs(UnmanagedType.LPWStr)] string id, out IMMDevice device);
        [PreserveSig] int RegisterEndpointNotificationCallback(IntPtr client);
        [PreserveSig] int UnregisterEndpointNotificationCallback(IntPtr client);
    }

    [ComImport, Guid("0BD7A1BE-7A1A-44DB-8397-CC5392387B5E"),
     InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
    internal interface IMMDeviceCollection
    {
        [PreserveSig] int GetCount(out uint count);
        [PreserveSig] int Item(uint index, out IMMDevice device);
    }

    [ComImport, Guid("D666063F-1587-4E43-81F1-B948E807363F"),
     InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
    internal interface IMMDevice
    {
        [PreserveSig] int Activate(ref Guid iid, int clsCtx, IntPtr activationParams,
            [MarshalAs(UnmanagedType.IUnknown)] out object iface);
        [PreserveSig] int OpenPropertyStore(int stgmRead, out IPropertyStore properties);
        [PreserveSig] int GetId([MarshalAs(UnmanagedType.LPWStr)] out string id);
        [PreserveSig] int GetState(out int state);
    }

    [ComImport, Guid("886D8EEB-8CF2-4446-8D02-CDBA1DBDCF99"),
     InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
    internal interface IPropertyStore
    {
        [PreserveSig] int GetCount(out uint count);
        [PreserveSig] int GetAt(uint index, out PropertyKey key);
        [PreserveSig] int GetValue(ref PropertyKey key, out PropVariant value);
        [PreserveSig] int SetValue(ref PropertyKey key, ref PropVariant value);
        [PreserveSig] int Commit();
    }

    [StructLayout(LayoutKind.Sequential)]
    internal struct PropertyKey
    {
        public Guid fmtid;
        public uint pid;
    }

    [StructLayout(LayoutKind.Sequential)]
    internal struct PropVariant
    {
        public ushort vt;
        public ushort wReserved1;
        public ushort wReserved2;
        public ushort wReserved3;
        public IntPtr pointerValue;

        public string? AsString() =>
            pointerValue == IntPtr.Zero ? null : Marshal.PtrToStringUni(pointerValue);
    }

    [ComImport, Guid("5CDF2C82-841E-4546-9722-0CF74078229A"),
     InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
    internal interface IAudioEndpointVolume
    {
        [PreserveSig] int RegisterControlChangeNotify(IntPtr notify);
        [PreserveSig] int UnregisterControlChangeNotify(IntPtr notify);
        [PreserveSig] int GetChannelCount(out uint count);
        [PreserveSig] int SetMasterVolumeLevel(float level, IntPtr eventContext);
        [PreserveSig] int SetMasterVolumeLevelScalar(float level, IntPtr eventContext);
        [PreserveSig] int GetMasterVolumeLevel(out float level);
        [PreserveSig] int GetMasterVolumeLevelScalar(out float level);
        [PreserveSig] int SetChannelVolumeLevel(uint channel, float level, IntPtr eventContext);
        [PreserveSig] int SetChannelVolumeLevelScalar(uint channel, float level, IntPtr eventContext);
        [PreserveSig] int GetChannelVolumeLevel(uint channel, out float level);
        [PreserveSig] int GetChannelVolumeLevelScalar(uint channel, out float level);
        [PreserveSig] int SetMute([MarshalAs(UnmanagedType.Bool)] bool mute, IntPtr eventContext);
        [PreserveSig] int GetMute([MarshalAs(UnmanagedType.Bool)] out bool mute);
        [PreserveSig] int GetVolumeStepInfo(out uint step, out uint stepSize);
        [PreserveSig] int VolumeStepUp(IntPtr eventContext);
        [PreserveSig] int VolumeStepDown(IntPtr eventContext);
        [PreserveSig] int QueryHardwareSupport(out uint mask);
        [PreserveSig] int GetVolumeRange(out float minLevel, out float maxLevel, out float step);
    }

    public static class AudioManager
    {
        private const int ERender = 0;
        private const int EMultimedia = 1;      // rôle « musique/média » = périphérique par défaut Windows
        private const int StgmRead = 0;         // STGM_READ
        private const int ClsctxAll = 0x17;     // CLSCTX_ALL
        private const int DeviceStateActive = 1;
        private const int PropertyKeyFriendlyNamePid = 14;

        private static readonly Guid DeviceFriendlyNameKey =
            new("A45C254E-DF1C-4EFD-8020-67D146A850E0");

        private static readonly Guid IAudioEndpointVolumeGuid =
            new("5CDF2C82-841E-4546-9722-0CF74078229A");

        private static IMMDeviceEnumerator CreateEnumerator() =>
            (IMMDeviceEnumerator)Activator.CreateInstance(
                Type.GetTypeFromCLSID(new Guid("BCDE0395-E52F-467C-8E3D-C4579291692E"))!)!;

        private static string? GetFriendlyName(IMMDevice device)
        {
            IPropertyStore? store = null;
            try
            {
                if (device.OpenPropertyStore(StgmRead, out store) != 0 || store == null) return null;
                var key = new PropertyKey { fmtid = DeviceFriendlyNameKey, pid = PropertyKeyFriendlyNamePid };
                if (store.GetValue(ref key, out var value) != 0) return null;
                return value.AsString();
            }
            catch { return null; }
            finally { if (store != null) Marshal.ReleaseComObject(store); }
        }

        private static IAudioEndpointVolume? GetDefaultEndpointVolume(out string? deviceName, out string? deviceId)
        {
            deviceName = null;
            deviceId = null;
            IMMDeviceEnumerator? enumerator = null;
            IMMDevice? device = null;
            try
            {
                enumerator = CreateEnumerator();
                if (enumerator.GetDefaultAudioEndpoint(ERender, EMultimedia, out device) != 0 || device == null)
                    return null;

                device.GetId(out deviceId);
                deviceName = GetFriendlyName(device);

                var iid = IAudioEndpointVolumeGuid;
                if (device.Activate(ref iid, ClsctxAll, IntPtr.Zero, out var iface) != 0 || iface == null)
                    return null;
                return (IAudioEndpointVolume)iface;
            }
            catch
            {
                return null;
            }
            finally
            {
                // Le endpoint volume reste utilisé par l'appelant ; le device/enumerator peut être libéré.
                if (device != null) Marshal.ReleaseComObject(device);
                if (enumerator != null) Marshal.ReleaseComObject(enumerator);
            }
        }

        private static int CheckHr(int hr, string operation)
        {
            if (hr != 0)
                throw new COMException($"Échec de l'opération audio « {operation} » (hr=0x{hr:X8}).", hr);
            return hr;
        }

        /// <summary>Lit le volume master, le mute et le périphérique de sortie par défaut (Core Audio réel).</summary>
        public static object ReadRealAudio()
        {
            var endpoint = GetDefaultEndpointVolume(out var deviceName, out var deviceId);
            if (endpoint == null)
                return new
                {
                    error = "Aucun périphérique de sortie audio actif sur cette session Windows.",
                    volume = (int?)null,
                    isMuted = (bool?)null,
                    deviceName = (string?)null,
                };

            try
            {
                CheckHr(endpoint.GetMasterVolumeLevelScalar(out var scalar), "GetMasterVolumeLevelScalar");
                CheckHr(endpoint.GetMute(out var muted), "GetMute");
                CheckHr(endpoint.GetChannelCount(out var channels), "GetChannelCount");

                return new
                {
                    volume = (int)Math.Round(Math.Clamp(scalar, 0f, 1f) * 100.0),
                    isMuted = muted,
                    deviceName,
                    deviceId,
                    channels,
                    note = "Volume/mute lus via Core Audio (IAudioEndpointVolume) — réel, pas simulé.",
                };
            }
            catch (Exception ex)
            {
                return new { error = ex.Message, volume = (int?)null, isMuted = (bool?)null, deviceName };
            }
            finally
            {
                Marshal.ReleaseComObject(endpoint);
            }
        }

        /// <summary>Définit le volume master (0–100) du périphérique de sortie par défaut.</summary>
        public static object SetRealVolume(int percent)
        {
            if (percent < 0 || percent > 100)
                return new { error = "Le volume doit être compris entre 0 et 100.", applied = false };

            var endpoint = GetDefaultEndpointVolume(out var deviceName, out _);
            if (endpoint == null)
                return new { error = "Aucun périphérique de sortie audio actif.", applied = false };

            try
            {
                CheckHr(endpoint.SetMasterVolumeLevelScalar(percent / 100f, IntPtr.Zero), "SetMasterVolumeLevelScalar");
                CheckHr(endpoint.GetMasterVolumeLevelScalar(out var readBack), "GetMasterVolumeLevelScalar");
                var applied = (int)Math.Round(Math.Clamp(readBack, 0f, 1f) * 100.0);
                return new { volume = applied, deviceName, applied = true };
            }
            catch (Exception ex)
            {
                return new { error = ex.Message, applied = false };
            }
            finally
            {
                Marshal.ReleaseComObject(endpoint);
            }
        }

        /// <summary>Passe le périphérique de sortie en mute ou non.</summary>
        public static object SetRealMute(bool muted)
        {
            var endpoint = GetDefaultEndpointVolume(out var deviceName, out _);
            if (endpoint == null)
                return new { error = "Aucun périphérique de sortie audio actif.", applied = false };

            try
            {
                CheckHr(endpoint.SetMute(muted, IntPtr.Zero), "SetMute");
                CheckHr(endpoint.GetMute(out var readBack), "GetMute");
                return new { isMuted = readBack, deviceName, applied = true };
            }
            catch (Exception ex)
            {
                return new { error = ex.Message, applied = false };
            }
            finally
            {
                Marshal.ReleaseComObject(endpoint);
            }
        }

        /// <summary>Liste les périphériques audio actifs (sortie + micro).</summary>
        public static object ListRealAudioDevices()
        {
            IMMDeviceEnumerator? enumerator = null;
            try
            {
                enumerator = CreateEnumerator();
                var devices = new List<object>();

                // eRender = 0 (sortie), eCapture = 1 (entrée/micro)
                foreach (var (dataFlow, flowName) in new[] { (0, "output"), (1, "input") })
                {
                    if (enumerator.EnumAudioEndpoints(dataFlow, DeviceStateActive, out var collection) != 0 || collection == null)
                        continue;
                    try
                    {
                        CheckHr(collection.GetCount(out var count), "GetCount");
                        for (uint i = 0; i < count; i++)
                        {
                            if (collection.Item(i, out var device) != 0 || device == null) continue;
                            try
                            {
                                device.GetId(out var id);
                                devices.Add(new { id, name = GetFriendlyName(device) ?? "Périphérique inconnu", flow = flowName });
                            }
                            finally { Marshal.ReleaseComObject(device); }
                        }
                    }
                    finally { Marshal.ReleaseComObject(collection); }
                }

                return new
                {
                    devices = devices.ToArray(),
                    note = "Périphériques audio réels (Core Audio IMMDeviceEnumerator).",
                };
            }
            catch (Exception ex)
            {
                return new { error = ex.Message, devices = Array.Empty<object>() };
            }
            finally
            {
                if (enumerator != null) Marshal.ReleaseComObject(enumerator);
            }
        }
    }
}
