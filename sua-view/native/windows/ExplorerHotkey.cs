using System;
using System.Diagnostics;
using System.IO;
using System.Runtime.InteropServices;
using System.Text;
using System.Threading;
using System.Windows.Forms;
using Microsoft.Win32;

// A per-user helper: intercept F3 only in Explorer with a supported selection.
internal static class ExplorerHotkey
{
    delegate IntPtr Hook(int code, IntPtr message, IntPtr data);
    static readonly Hook callback = Keyboard;
    static IntPtr hook;
    static bool pressed;
    static readonly string executable = Application.ExecutablePath;
    [DllImport("user32.dll")] static extern IntPtr SetWindowsHookEx(int id, Hook proc, IntPtr module, uint thread);
    [DllImport("user32.dll")] static extern bool UnhookWindowsHookEx(IntPtr handle);
    [DllImport("user32.dll")] static extern IntPtr CallNextHookEx(IntPtr handle, int code, IntPtr message, IntPtr data);
    [DllImport("user32.dll")] static extern IntPtr GetForegroundWindow();
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] static extern int GetClassName(IntPtr hwnd, StringBuilder text, int count);
    [DllImport("user32.dll")] static extern short GetAsyncKeyState(int key);
    [DllImport("kernel32.dll", CharSet = CharSet.Unicode)] static extern IntPtr GetModuleHandle(string name);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] static extern IntPtr FindWindowEx(IntPtr parent, IntPtr after, string name, string title);
    [ComImport, Guid("6D5140C1-7436-11CE-8034-00AA006009FA"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
    interface IServiceProvider { [PreserveSig] int QueryService(ref Guid service, ref Guid iid, [MarshalAs(UnmanagedType.Interface)] out object result); }
    [ComImport, Guid("00000114-0000-0000-C000-000000000046"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
    interface IOleWindow { void GetWindow(out IntPtr hwnd); void ContextSensitiveHelp(bool enter); }

    static bool ActiveTab(object window, IntPtr frame)
    {
        IntPtr active = FindWindowEx(frame, IntPtr.Zero, "ShellTabWindowClass", null);
        if (active == IntPtr.Zero) return true; // Windows 10 has no tabs.
        Guid service = new Guid("4C96BE40-915C-11CF-99D3-00AA004AE837");
        Guid iid = new Guid("00000114-0000-0000-C000-000000000046");
        object browser;
        if (((IServiceProvider)window).QueryService(ref service, ref iid, out browser) != 0) return false;
        try { IntPtr tab; ((IOleWindow)browser).GetWindow(out tab); return tab == active; }
        finally { Marshal.ReleaseComObject(browser); }
    }

    [STAThread] static int Main(string[] args)
    {
        if (args.Length > 0 && (args[0] == "--enable" || args[0] == "--disable")) {
            using (var key = Registry.CurrentUser.CreateSubKey(@"Software\Microsoft\Windows\CurrentVersion\Run")) {
                if (args[0] == "--disable") key.DeleteValue("SuaViewExplorerF3", false);
                else key.SetValue("SuaViewExplorerF3", "\"" + executable + "\"");
            }
            if (args[0] == "--disable") {
                foreach (var process in Process.GetProcessesByName(Path.GetFileNameWithoutExtension(executable))) {
                    if (process.Id != Process.GetCurrentProcess().Id && process.MainModule.FileName == executable) process.Kill();
                    process.Dispose();
                }
                return 0;
            }
        }
        bool created;
        using (var mutex = new Mutex(true, "Local\\SuaViewExplorerF3", out created)) {
            if (!created) return 0;
            hook = SetWindowsHookEx(13, callback, GetModuleHandle(null), 0);
            if (hook == IntPtr.Zero) return 1;
            using (var tray = new NotifyIcon()) {
                tray.Icon = System.Drawing.SystemIcons.Application;
                tray.Text = "Sua View — Explorer F3";
                tray.ContextMenu = new ContextMenu(new[] {
                    new MenuItem("Disable F3 and startup", delegate { Process.Start(executable, "--disable"); }),
                    new MenuItem("Exit", delegate { Application.Exit(); }) });
                tray.Visible = true;
                try { Application.Run(); }
                finally { tray.Visible = false; UnhookWindowsHookEx(hook); }
            }
        }
        return 0;
    }
    static IntPtr Keyboard(int code, IntPtr message, IntPtr data)
    {
        if (code >= 0 && Marshal.ReadInt32(data) == 0x72) {
            bool down = message.ToInt32() == 0x100;
            if (message.ToInt32() == 0x101 && pressed) { pressed = false; return (IntPtr)1; }
            if (down && pressed) return (IntPtr)1;
            if (down && GetAsyncKeyState(0x10) >= 0 && GetAsyncKeyState(0x11) >= 0 && GetAsyncKeyState(0x12) >= 0) {
                var hwnd = GetForegroundWindow(); var name = new StringBuilder(128);
                GetClassName(hwnd, name, name.Capacity);
                if (name.ToString() == "CabinetWClass" || name.ToString() == "ExploreWClass") {
                    // Never block the low-level hook on Explorer COM calls.
                    pressed = true;
                    var worker = new Thread(delegate() { OpenSelection(hwnd); });
                    worker.SetApartmentState(ApartmentState.STA); worker.IsBackground = true; worker.Start();
                    return (IntPtr)1;
                }
            }
        }
        return CallNextHookEx(hook, code, message, data);
    }
    static void OpenSelection(IntPtr hwnd)
    {
        object shell = null, windows = null;
        try {
            shell = Activator.CreateInstance(Type.GetTypeFromProgID("Shell.Application"));
            dynamic app = shell; windows = app.Windows();
            foreach (dynamic window in (dynamic)windows) {
                try {
                    if ((long)window.HWND != hwnd.ToInt64()) continue;
                    if (!ActiveTab((object)window, hwnd)) continue;
                    dynamic selected = window.Document.SelectedItems();
                    if (selected.Count == 0) return;
                    string path = selected.Item(0).Path;
                    string extension = Path.GetExtension(path).ToLowerInvariant();
                    if (Directory.Exists(path) || extension == ".zip" || extension == ".cbz" || extension == ".jpg" || extension == ".jpeg" || extension == ".png")
                        Process.Start(new ProcessStartInfo(Path.Combine(Path.GetDirectoryName(executable), "SuaView.exe"), "\"" + path + "\"") { UseShellExecute = false });
                    return;
                } catch (COMException) { }
            }
        } catch (Exception ex) { Debug.WriteLine(ex); }
        finally {
            if (windows != null) Marshal.FinalReleaseComObject(windows);
            if (shell != null) Marshal.FinalReleaseComObject(shell);
        }
    }
}
