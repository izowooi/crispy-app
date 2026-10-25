using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Runtime.InteropServices;
using System.Text;
using System.Threading.Tasks;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Input;
using System.Windows.Media;
using System.Windows.Media.Imaging;
using Microsoft.Win32;

// Native Windows UI; the same Rust core owns archive IO on both platforms.
internal static class Core
{
    const string Dll = "sua_view_core.dll";
    [DllImport(Dll, CallingConvention = CallingConvention.Cdecl)] internal static extern IntPtr sua_book_open(byte[] path, out IntPtr error);
    [DllImport(Dll, CallingConvention = CallingConvention.Cdecl)] internal static extern UIntPtr sua_book_count(IntPtr book);
    [DllImport(Dll, CallingConvention = CallingConvention.Cdecl)] internal static extern UIntPtr sua_book_initial_page(IntPtr book);
    [DllImport(Dll, CallingConvention = CallingConvention.Cdecl)] internal static extern IntPtr sua_book_page_name(IntPtr book, UIntPtr index);
    [DllImport(Dll, CallingConvention = CallingConvention.Cdecl)] internal static extern IntPtr sua_book_read_page(IntPtr book, UIntPtr index, out UIntPtr length, out IntPtr error);
    [DllImport(Dll, CallingConvention = CallingConvention.Cdecl)] internal static extern void sua_bytes_free(IntPtr bytes, UIntPtr length);
    [DllImport(Dll, CallingConvention = CallingConvention.Cdecl)] internal static extern void sua_string_free(IntPtr text);
    [DllImport(Dll, CallingConvention = CallingConvention.Cdecl)] internal static extern void sua_book_close(IntPtr book);
    [DllImport(Dll, CallingConvention = CallingConvention.Cdecl)] internal static extern byte sua_page_is_landscape(double width, double height);
    [DllImport(Dll, CallingConvention = CallingConvention.Cdecl)] internal static extern byte sua_can_pair(UIntPtr index, UIntPtr end, byte spread, byte cover, byte firstLandscape, byte secondLandscape);
    [DllImport(Dll, CallingConvention = CallingConvention.Cdecl)] internal static extern UIntPtr sua_next_start(UIntPtr index, UIntPtr count, UIntPtr displayed);
    [DllImport(Dll, CallingConvention = CallingConvention.Cdecl)] internal static extern UIntPtr sua_previous_start(UIntPtr index, byte spread, byte cover, int previousLandscape, int earlierLandscape);
    [DllImport(Dll, CallingConvention = CallingConvention.Cdecl)] internal static extern byte sua_path_is_image(byte[] path);
    internal static byte Flag(bool value) { return (byte)(value ? 1 : 0); }
    internal static byte Landscape(BitmapSource image) { return sua_page_is_landscape(image.PixelWidth, image.PixelHeight); }
    internal static string Text(IntPtr p)
    {
        if (p == IntPtr.Zero) return "Unknown reader error.";
        try { int n = 0; while (Marshal.ReadByte(p, n) != 0) n++; byte[] b = new byte[n]; Marshal.Copy(p, b, 0, n); return Encoding.UTF8.GetString(b); }
        finally { sua_string_free(p); }
    }
}

internal sealed class Reader : IDisposable
{
    IntPtr book;
    readonly Dictionary<int, BitmapSource> cache = new Dictionary<int, BitmapSource>();
    internal int Count, Initial;
    internal Reader(string path)
    {
        IntPtr error;
        book = Core.sua_book_open(Encoding.UTF8.GetBytes(path + "\0"), out error);
        if (book == IntPtr.Zero) throw new IOException(Core.Text(error));
        Count = checked((int)Core.sua_book_count(book).ToUInt64());
        Initial = checked((int)Core.sua_book_initial_page(book).ToUInt64());
    }
    internal BitmapSource Page(int index)
    {
        BitmapSource cached;
        if (cache.TryGetValue(index, out cached)) return cached;
        UIntPtr size; IntPtr error;
        IntPtr p = Core.sua_book_read_page(book, (UIntPtr)index, out size, out error);
        if (p == IntPtr.Zero) throw new IOException(Core.Text(error));
        byte[] bytes;
        try { bytes = new byte[checked((int)size.ToUInt64())]; Marshal.Copy(p, bytes, 0, bytes.Length); }
        finally { Core.sua_bytes_free(p, size); }
        using (var stream = new MemoryStream(bytes, false))
        {
            var decoder = BitmapDecoder.Create(stream, BitmapCreateOptions.PreservePixelFormat, BitmapCacheOption.None);
            var frame = decoder.Frames[0];
            if ((long)frame.PixelWidth * frame.PixelHeight > 20000000) throw new IOException("Image exceeds the Windows 20 megapixel limit.");
            // Materialize before disposing the input stream. Cache at most two 80 MiB pages.
            var converted = new FormatConvertedBitmap(frame, PixelFormats.Bgra32, null, 0);
            var image = new WriteableBitmap(converted); image.Freeze();
            cache[index] = image;
            foreach (int old in cache.Keys.Where(k => k != index && k != index - 1).ToArray()) cache.Remove(old);
            return image;
        }
    }
    internal string Name(int index) { return Core.Text(Core.sua_book_page_name(book, (UIntPtr)index)); }
    int Aspect(int index)
    {
        try { return Core.Landscape(Page(index)); }
        catch (IOException) { return -1; }
        catch (NotSupportedException) { return -1; }
        catch (System.IO.FileFormatException) { return -1; }
    }
    internal int PreviousStart(int index, bool dual)
    {
        int previousAspect = index > 0 && dual ? Aspect(index - 1) : -1;
        int earlierAspect = index > 1 && previousAspect == 0 ? Aspect(index - 2) : -1;
        return checked((int)Core.sua_previous_start((UIntPtr)index, Core.Flag(dual), 1, previousAspect, earlierAspect).ToUInt64());
    }
    public void Dispose() { if (book != IntPtr.Zero) Core.sua_book_close(book); book = IntPtr.Zero; cache.Clear(); }
}

internal sealed partial class Viewer : Window
{
    readonly System.Threading.SemaphoreSlim io = new System.Threading.SemaphoreSlim(1);
    readonly Dictionary<string, int> positions = new Dictionary<string, int>(StringComparer.OrdinalIgnoreCase);
    readonly string historyPath = Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "Sua View", "positions.txt");
    Reader reader;
    string bookPath;
    int index, shown = 1, request;
    int? spanEnd;
    bool dual, rtl, fitWidth, fullscreen, closing;
    double zoom = 1;
    WindowState previousState;
    readonly Stack<int> previous = new Stack<int>();
    Task movement = Task.FromResult(0);
    internal Viewer()
    {
        ConfigureAppearance();
        AllowDrop = true;
        DragOver += delegate(object s, DragEventArgs e) { e.Effects = e.Data.GetDataPresent(DataFormats.FileDrop) ? DragDropEffects.Copy : DragDropEffects.None; e.Handled = true; };
        Drop += delegate(object s, DragEventArgs e) { var paths = e.Data.GetData(DataFormats.FileDrop) as string[]; if (paths != null && paths.Length > 0) Open(paths[0]); };
        PreviewKeyDown += Keys;
        SizeChanged += delegate { LayoutPages(); };
        Closing += async delegate(object s, System.ComponentModel.CancelEventArgs e) {
            if (closing) return; e.Cancel = true; closing = true; ++request; Save();
            await io.WaitAsync(); try { if (reader != null) reader.Dispose(); reader = null; } finally { io.Release(); }
            await System.Windows.Threading.Dispatcher.Yield();
            Close();
        };
        try { if (File.Exists(historyPath)) foreach (string line in File.ReadAllLines(historyPath)) { string[] p = line.Split('\t'); int n; if (p.Length == 2 && int.TryParse(p[1], out n)) positions[Encoding.UTF8.GetString(Convert.FromBase64String(p[0]))] = n; } } catch { }
    }
    void Save()
    {
        if (Environment.GetEnvironmentVariable("SUA_VIEW_TESTING") == "1" || bookPath == null) return;
        positions.Remove(bookPath); positions[bookPath] = index;
        try { Directory.CreateDirectory(Path.GetDirectoryName(historyPath)); File.WriteAllLines(historyPath, positions.Reverse().Take(50).Select(p => Convert.ToBase64String(Encoding.UTF8.GetBytes(p.Key)) + "\t" + p.Value).ToArray()); } catch { }
    }
    void OpenDialog() { var d = new OpenFileDialog { Filter = "만화책·이미지|*.zip;*.cbz;*.jpg;*.jpeg;*.png;*.bmp;*.gif;*.tif;*.tiff|모든 파일|*.*" }; if (d.ShowDialog(this) == true) Open(d.FileName); }
    void OpenFolder() { using (var d = new System.Windows.Forms.FolderBrowserDialog()) { if (d.ShowDialog() == System.Windows.Forms.DialogResult.OK) Open(d.SelectedPath); } }
    internal async void Open(string path) { await OpenAsync(path); }
    async Task OpenAsync(string path)
    {
        if (closing) return;
        Save(); int token = ++request; ShowMessage("열고 있습니다…");
        await io.WaitAsync();
        try
        {
            if (token != request || closing) return;
            Reader opened = await Task.Run(() => new Reader(path));
            if (token != request || closing) { opened.Dispose(); return; }
            if (reader != null) reader.Dispose(); reader = opened; bookPath = Path.GetFullPath(path);
            int saved;
            bool isImage = Core.sua_path_is_image(Encoding.UTF8.GetBytes(bookPath + "\0")) != 0;
            index = !isImage && positions.TryGetValue(bookPath, out saved) ? Math.Max(0, Math.Min(saved, reader.Count - 1)) : reader.Initial;
            ResetNavigation(); zoom = 1; Title = Path.GetFileName(path) + " — Sua View";
        }
        catch (Exception ex) { if (token == request) ShowMessage(ex.Message); return; }
        finally { io.Release(); }
        await RenderAsync();
    }
    internal async void Render() { await RenderAsync(); }
    async Task RenderAsync()
    {
        if (reader == null || closing) return;
        int token = ++request, at = index, end = Math.Min(reader.Count, spanEnd ?? reader.Count); bool pair = dual;
        await io.WaitAsync();
        try
        {
            if (token != request || closing) return;
            var result = await Task.Run(() => {
                var images = new List<BitmapSource> { reader.Page(at) };
                byte firstLandscape = Core.Landscape(images[0]);
                if (Core.sua_can_pair((UIntPtr)at, (UIntPtr)end, Core.Flag(pair), 1, firstLandscape, 0) != 0)
                {
                    // A corrupt neighbour must not hide the healthy first page.
                    try {
                        var next = reader.Page(at + 1);
                        if (Core.sua_can_pair((UIntPtr)at, (UIntPtr)end, Core.Flag(pair), 1, firstLandscape, Core.Landscape(next)) != 0) images.Add(next);
                    }
                    catch (IOException) { }
                    catch (NotSupportedException) { }
                    catch (System.IO.FileFormatException) { }
                }
                return images;
            });
            if (token != request || closing) return;
            shown = result.Count; pages.Children.Clear();
            if (rtl) result.Reverse();
            foreach (var image in result) pages.Children.Add(new Image { Source = image, Stretch = Stretch.Uniform, Margin = new Thickness(1, 0, 1, 0) });
            LayoutPages(); scroll.ScrollToHome();
            status.Text = (index + 1) + (shown == 2 ? "–" + (index + 2) : "") + " / " + reader.Count + "  ·  " + reader.Name(index) + "  ·  " + (rtl ? "우→좌" : "좌→우");
            UpdateHud();
            Save();
        }
        catch (Exception ex) { if (token == request) { shown = 1; pages.Children.Clear(); ShowMessage(ex.Message + "\n\nSpace로 다음 쪽을 읽을 수 있습니다."); RevealHud(); } }
        finally { io.Release(); }
    }
    void LayoutPages()
    {
        if (pages.Children.Count == 0) return;
        double width = Math.Max(100, scroll.ActualWidth - 24), height = Math.Max(100, scroll.ActualHeight - 24);
        double aspect = pages.Children.Cast<Image>().Sum(i => ((BitmapSource)i.Source).PixelWidth / (double)((BitmapSource)i.Source).PixelHeight);
        double h = (fitWidth ? width / aspect : Math.Min(height, width / aspect)) * zoom;
        foreach (Image image in pages.Children) { var b = (BitmapSource)image.Source; image.Height = h; image.Width = h * b.PixelWidth / b.PixelHeight; }
    }
    void Next() { movement = Move(movement, true); }
    void Back() { movement = Move(movement, false); }
    void ResetNavigation() { previous.Clear(); spanEnd = null; }
    async Task Move(Task prior, bool forward)
    {
        await prior;
        if (reader == null || closing) return;
        if (forward) {
            int next = checked((int)Core.sua_next_start((UIntPtr)index, (UIntPtr)reader.Count, (UIntPtr)shown).ToUInt64());
            if (next == index) return;
            previous.Push(index); index = next; spanEnd = null;
        }
        else {
            int before = index;
            if (previous.Count > 0) index = previous.Pop();
            else {
                int token = request;
                await io.WaitAsync();
                try {
                    if (closing || token != request) return;
                    int destination = await Task.Run(() => reader.PreviousStart(before, dual));
                    if (closing || token != request) return;
                    index = destination;
                }
                finally { io.Release(); }
            }
            spanEnd = before;
        }
        await RenderAsync();
    }
    void Scale(double factor) { zoom = Math.Max(0.25, Math.Min(8, zoom * factor)); LayoutPages(); }
    void Fullscreen()
    {
        fullscreen = !fullscreen;
        if (fullscreen) { previousState = WindowState; WindowState = WindowState.Normal; WindowStyle = WindowStyle.None; chrome.CaptionHeight = 0; titleBar.Visibility = Visibility.Collapsed; WindowState = WindowState.Maximized; }
        else { WindowState = WindowState.Normal; WindowStyle = WindowStyle.SingleBorderWindow; chrome.CaptionHeight = 34; titleBar.Visibility = Visibility.Visible; WindowState = previousState; }
        RevealHud();
    }
    void Go()
    {
        if (reader == null) return;
        var box = new TextBox { Text = (index + 1).ToString(), Margin = new Thickness(0, 16, 0, 12), Padding = new Thickness(8), FontSize = 22, TextAlignment = TextAlignment.Center, Background = Appearance.Brush("button"), Foreground = Appearance.Brush("textPrimary"), BorderBrush = Appearance.Brush("textMuted") };
        var panel = new StackPanel { Margin = new Thickness(24) };
        panel.Children.Add(new TextBlock { Text = "쪽 번호로 이동", FontSize = 16, FontWeight = FontWeights.SemiBold }); panel.Children.Add(box);
        panel.Children.Add(new TextBlock { Text = "1–" + reader.Count + "  ·  Enter로 이동", FontSize = 12, Foreground = Appearance.Brush("textSecondary"), HorizontalAlignment = HorizontalAlignment.Center });
        var dialog = ThemedDialog("쪽 번호로 이동", panel, 320);
        box.KeyDown += delegate(object s, KeyEventArgs e) { if (e.Key == Key.Enter) { int n; if (int.TryParse(box.Text, out n) && n > 0 && n <= reader.Count) { ResetNavigation(); index = n - 1; dialog.Close(); Render(); } } else if (e.Key == Key.Escape) dialog.Close(); };
        dialog.Loaded += delegate { box.Focus(); box.SelectAll(); }; dialog.ShowDialog();
    }
    void Help()
    {
        var panel = new StackPanel { Margin = new Thickness(28) };
        panel.Children.Add(new TextBlock { Text = "편안하게 읽기", FontSize = 20, FontWeight = FontWeights.SemiBold, Margin = new Thickness(0, 0, 0, 18) });
        panel.Children.Add(new TextBlock { Text = "Ctrl+O: 책 열기 · Ctrl+Shift+O: 이미지 폴더\n← / →: 읽기 방향에 따라 이동\nSpace / Shift+Space: 다음 / 이전\nHome / End: 처음 / 마지막\nD: 한 쪽·두 쪽 · R: 읽기 방향\nW: 너비 맞춤 · 0: 화면 맞춤\n+ / −, Ctrl+휠: 확대·축소\n휠·드래그: 확대된 페이지 이동\nG: 쪽 번호 · F, 더블 클릭: 전체 화면\nEsc: 전체 화면 종료\n\n좌우 영역을 클릭하거나 휠로 페이지를 넘깁니다.\n마우스를 움직이면 하단 조작부가 나타납니다.", FontSize = 13, LineHeight = 25, Foreground = Appearance.Brush("textSecondary") });
        panel.Children.Add(new TextBlock { Text = "ZIP/CBZ · JPG/PNG/GIF/BMP/TIFF\nRAR/7z 및 WebP/HEIC/AVIF는 미지원", FontSize = 11, LineHeight = 19, Foreground = Appearance.Brush("textMuted"), Margin = new Thickness(0, 20, 0, 0) });
        ThemedDialog("Sua View 단축키", panel, 440).ShowDialog();
    }
    void Keys(object sender, KeyEventArgs e)
    {
        Key k = e.Key == Key.ImeProcessed ? e.ImeProcessedKey : e.Key;
        bool ctrl = (Keyboard.Modifiers & ModifierKeys.Control) != 0, shift = (Keyboard.Modifiers & ModifierKeys.Shift) != 0;
        if (ctrl && k == Key.O) { if (shift) OpenFolder(); else OpenDialog(); }
        else if (k == Key.Right) { if (rtl) Back(); else Next(); }
        else if (k == Key.Left) { if (rtl) Next(); else Back(); }
        else if (k == Key.Space) { if (shift) Back(); else Next(); }
        else if (k == Key.PageDown) Next(); else if (k == Key.PageUp) Back();
        else if (k == Key.Home || k == Key.End) { if (reader != null) { ResetNavigation(); index = k == Key.Home ? 0 : reader.Count - 1; Render(); } }
        else if (k == Key.D) { dual = !dual; ResetNavigation(); Render(); }
        else if (k == Key.R) { rtl = !rtl; Render(); }
        else if (k == Key.W) { fitWidth = true; zoom = 1; Render(); }
        else if (k == Key.D0 || k == Key.NumPad0) { fitWidth = false; zoom = 1; Render(); }
        else if (k == Key.OemPlus || k == Key.Add) Scale(1.2);
        else if (k == Key.OemMinus || k == Key.Subtract) Scale(1 / 1.2);
        else if (k == Key.G) Go(); else if (k == Key.F || k == Key.F11) Fullscreen();
        else if (k == Key.Escape && fullscreen) Fullscreen(); else if (k == Key.F1 || k == Key.OemQuestion) Help();
        else return;
        e.Handled = true;
    }
    [STAThread] internal static int Main(string[] args)
    {
        try
        {
            if (args.Length == 2 && args[0] == "--smoke-test") return Smoke(args[1]);
            var app = new Application(); var window = new Viewer();
            if (args.Length > 0) window.Loaded += delegate { window.Open(args[0]); };
            return app.Run(window);
        }
        catch (Exception ex) { MessageBox.Show(ex.ToString(), "Sua View"); return 1; }
    }
    static int Smoke(string path)
    {
        // Runs the release DLL, UTF-8 FFI, real Windows decoder, and WPF layout.
        using (var r = new Reader(path))
        {
            if (r.Count < 3) throw new Exception("Smoke fixture requires at least three pages.");
            foreach (int n in new[] { 0, 1, r.Count - 1, 0 })
            {
                var image = r.Page(n);
                if (image.PixelWidth < 1 || image.PixelHeight < 1 || r.Name(n).Length == 0) throw new Exception("Invalid decoded page.");
            }
        }
        int code = 1;
        var app = new Application(); var window = new Viewer();
        string evidence = Environment.GetEnvironmentVariable("SUA_VIEW_SMOKE_OUTPUT");
        if (String.IsNullOrEmpty(evidence)) throw new Exception("Set SUA_VIEW_SMOKE_OUTPUT to a local evidence directory.");
        Directory.CreateDirectory(evidence);
        window.Loaded += async delegate {
            try
            {
                await Task.Delay(150); window.UpdateLayout();
                if (window.empty.Visibility != Visibility.Visible || window.hud.Visibility != Visibility.Collapsed) throw new Exception("Empty state must show the book mark without reading controls.");
                Capture(window, Path.Combine(evidence, "empty.png"));
                await window.OpenAsync(path);
                if (window.reader == null || window.pages.Children.Count != 1) throw new Exception("First page failed: " + window.status.Text);
                window.dual = true; await window.RenderAsync();
                if (window.shown != 1) throw new Exception("Cover must be single.");
                window.Next(); await window.movement;
                if (window.index != 1 || window.shown != 2) throw new Exception("Two page spread failed.");
                window.Next(); window.Next(); window.Back(); await window.movement;
                if (window.index != 3) throw new Exception("Queued navigation lost or duplicated pages.");
                window.rtl = true; await window.RenderAsync();
                var first = (BitmapSource)((Image)window.pages.Children[0]).Source;
                if (!Object.ReferenceEquals(first, window.reader.Page(window.index + 1))) throw new Exception("RTL order failed.");
                window.ResetNavigation(); window.index = 0; await window.RenderAsync();
                var visited = new HashSet<int>();
                for (int step = 0; step < window.reader.Count + 1; step++) {
                    for (int p = window.index; p < window.index + window.shown; p++) if (!visited.Add(p)) throw new Exception("Spread repeated a page.");
                    int before = window.index; window.Next(); await window.movement;
                    if (before == window.index) break;
                }
                if (visited.Count != window.reader.Count) throw new Exception("Spread traversal skipped pages.");
                for (int target = 1; target < window.reader.Count; target++) {
                    window.ResetNavigation(); window.index = target; await window.RenderAsync();
                    window.Back(); await window.movement;
                    if (window.index + window.shown != target) throw new Exception("Previous spread overlapped a jump or skipped a page.");
                    window.Next(); await window.movement;
                    if (window.index != target) throw new Exception("Next failed to reverse previous after jump.");
                }
                window.ResetNavigation();
                window.index = 5; await window.RenderAsync();
                if (window.shown != 1) throw new Exception("Landscape must be single.");
                window.fitWidth = true; window.Scale(1.2); window.Fullscreen(); window.Fullscreen();
                window.fitWidth = false; window.zoom = 1;
                window.index = 1; await window.RenderAsync(); window.UpdateLayout();
                if (!window.hideHud.IsEnabled || window.empty.Visibility != Visibility.Collapsed) throw new Exception("Reading must replace the welcome view and schedule HUD hiding.");
                window.RevealHud(); await Task.Delay(150); Capture(window, Path.Combine(evidence, "window.png"));
                window.ShowOptions(); await Task.Delay(100);
                if (!window.options.IsOpen || window.options.Items.Count < 10) throw new Exception("Reading options must remain accessible.");
                Capture(window.options, Path.Combine(evidence, "options.png"));
                var menuClosed = new TaskCompletionSource<bool>();
                window.options.Closed += delegate { menuClosed.TrySetResult(true); };
                window.options.IsOpen = false; await menuClosed.Task;
                await System.Windows.Threading.Dispatcher.Yield(System.Windows.Threading.DispatcherPriority.ContextIdle);
                window.HideHud(); await Task.Delay(250);
                if (window.hud.Opacity > 0.01 || window.hud.IsHitTestVisible) throw new Exception("Hidden reading controls must not obscure or intercept the page: opacity=" + window.hud.Opacity + ", hitTest=" + window.hud.IsHitTestVisible);
                Capture(window, Path.Combine(evidence, "reading-clean.png"));
                window.RevealHud(); await Task.Delay(150);
                if (window.hud.Opacity < 0.99 || !window.hud.IsHitTestVisible) throw new Exception("Reading controls must reappear on demand.");
                window.Width = 520; window.Height = 420; window.UpdateLayout();
                if (window.hud.ActualWidth > window.ActualWidth - 20 || window.pageName.Visibility != Visibility.Collapsed) throw new Exception("Reading controls must fit the minimum window size.");
                Capture(window, Path.Combine(evidence, "compact.png"));
                File.WriteAllText(Path.Combine(evidence, "result.txt"), "PASS: Rust FFI, Unicode path, Windows decoding, cover, full spread traversal, queued next/next/back, jump/back/next at every page, RTL, landscape, zoom, fullscreen, empty book mark, minimal chrome, options menu, HUD hide/reveal, compact layout, WPF renders");
                code = 0;
            }
            catch (Exception ex) { File.WriteAllText(Path.Combine(evidence, "result.txt"), ex.ToString()); }
            finally { window.Close(); }
        };
        app.Run(window);
        return code;
    }
    static void Capture(FrameworkElement element, string path)
    {
        element.UpdateLayout();
        var bitmap = new RenderTargetBitmap((int)Math.Ceiling(element.ActualWidth), (int)Math.Ceiling(element.ActualHeight), 96, 96, PixelFormats.Pbgra32);
        bitmap.Render(element);
        var encoder = new PngBitmapEncoder(); encoder.Frames.Add(BitmapFrame.Create(bitmap));
        using (var file = File.Create(path)) encoder.Save(file);
    }
}
