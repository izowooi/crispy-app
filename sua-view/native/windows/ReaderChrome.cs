using System;
using System.Runtime.InteropServices;
using System.Threading.Tasks;
using System.Windows;
using System.Windows.Automation;
using System.Windows.Controls;
using System.Windows.Input;
using System.Windows.Interop;
using System.Windows.Media;
using System.Windows.Media.Animation;
using System.Windows.Media.Effects;
using System.Windows.Shell;
using System.Windows.Threading;

internal sealed partial class Viewer
{
    readonly Grid root = new Grid();
    readonly Grid surface = new Grid();
    readonly Grid titleBar = new Grid { Height = 34 };
    readonly TextBlock caption = new TextBlock();
    readonly StackPanel empty = new StackPanel { HorizontalAlignment = HorizontalAlignment.Center, VerticalAlignment = VerticalAlignment.Center };
    readonly TextBlock status = new TextBlock { TextAlignment = TextAlignment.Center, TextWrapping = TextWrapping.Wrap, MaxWidth = 600, Margin = new Thickness(32), HorizontalAlignment = HorizontalAlignment.Center, VerticalAlignment = VerticalAlignment.Center, IsHitTestVisible = false, Visibility = Visibility.Collapsed };
    readonly StackPanel pages = new StackPanel { Orientation = Orientation.Horizontal, HorizontalAlignment = HorizontalAlignment.Center, VerticalAlignment = VerticalAlignment.Center };
    readonly ScrollViewer scroll = new ScrollViewer { HorizontalScrollBarVisibility = ScrollBarVisibility.Hidden, VerticalScrollBarVisibility = ScrollBarVisibility.Hidden, PanningMode = PanningMode.Both, Focusable = false };
    readonly Border hud = new Border();
    readonly TextBlock pageNumber = new TextBlock();
    readonly TextBlock pageName = new TextBlock();
    readonly Slider slider = new Slider { Minimum = 0, SmallChange = 1, LargeChange = 1, IsSnapToTickEnabled = true, TickFrequency = 1, Focusable = false };
    readonly DispatcherTimer hideHud = new DispatcherTimer();
    WindowChrome chrome;
    ContextMenu options;
    Button backButton, nextButton;
    bool updatingSlider, dragging, doubleClicked;
    Point? downPoint;
    Point? lastPointer;
    double dragHorizontal, dragVertical;
    DateTime lastWheel = DateTime.MinValue;

    [DllImport("dwmapi.dll")] static extern int DwmSetWindowAttribute(IntPtr window, int attribute, ref int value, int size);

    void ConfigureAppearance()
    {
        Resources.MergedDictionaries.Add(Appearance.Styles());
        Title = "Sua View"; Width = 1080; Height = 790; MinWidth = 520; MinHeight = 420;
        Background = Appearance.Brush("canvas"); Foreground = Appearance.Brush("textPrimary");
        FontFamily = new FontFamily("Segoe UI"); UseLayoutRounding = true; SnapsToDevicePixels = true;
        WindowStartupLocation = WindowStartupLocation.CenterScreen; Icon = Appearance.BookMark();
        chrome = new WindowChrome { CaptionHeight = 34, ResizeBorderThickness = new Thickness(6), GlassFrameThickness = new Thickness(0), CornerRadius = new CornerRadius(8), UseAeroCaptionButtons = false };
        WindowChrome.SetWindowChrome(this, chrome);
        SourceInitialized += delegate {
            int enabled = 1, rounded = 2; IntPtr handle = new WindowInteropHelper(this).Handle;
            DwmSetWindowAttribute(handle, 20, ref enabled, 4); DwmSetWindowAttribute(handle, 33, ref rounded, 4);
        };
        root.RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });
        root.RowDefinitions.Add(new RowDefinition { Height = new GridLength(1, GridUnitType.Star) });
        Content = root; root.Children.Add(titleBar); Grid.SetRow(surface, 1); root.Children.Add(surface);
        ConfigureTitleBar();
        scroll.Content = pages; surface.Children.Add(scroll); surface.Children.Add(empty); surface.Children.Add(status);
        status.Foreground = Appearance.Brush("textSecondary"); status.FontSize = 14;
        ConfigureEmpty(); ConfigureHud();
        MouseMove += delegate(object sender, MouseEventArgs e) {
            Point point = e.GetPosition(surface);
            if (!lastPointer.HasValue || (point - lastPointer.Value).Length > 0.5) { lastPointer = point; RevealHud(); }
        };
        PreviewKeyDown += delegate { RevealHud(); };
        hideHud.Interval = TimeSpan.FromSeconds(Appearance.Number("hudHideSeconds"));
        hideHud.Tick += delegate { if (hud.IsMouseOver || slider.IsMouseCaptureWithin || (options != null && options.IsOpen)) return; HideHud(); };
        Deactivated += delegate { if (options == null || !options.IsOpen) HideHud(); };
        Closed += delegate { hideHud.Stop(); };
        caption.SetBinding(TextBlock.TextProperty, new System.Windows.Data.Binding("Title") { Source = this });
        surface.ContextMenuOpening += delegate(object sender, ContextMenuEventArgs e) { options = BuildOptions(); surface.ContextMenu = options; };
        surface.ContextMenu = BuildOptions();
        SizeChanged += delegate { hud.Width = Math.Max(240, Math.Min(Appearance.Number("hudWidth"), ActualWidth - 28)); pageName.Visibility = ActualWidth < 650 ? Visibility.Collapsed : Visibility.Visible; };
        ConfigurePointer();
    }

    void ConfigureTitleBar()
    {
        titleBar.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        titleBar.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        caption.Text = "Sua View"; caption.FontSize = 12; caption.Foreground = Appearance.Brush("textSecondary");
        caption.VerticalAlignment = VerticalAlignment.Center; caption.Margin = new Thickness(14, 0, 0, 0); caption.TextTrimming = TextTrimming.CharacterEllipsis;
        titleBar.Children.Add(caption);
        var controls = new StackPanel { Orientation = Orientation.Horizontal }; Grid.SetColumn(controls, 1); titleBar.Children.Add(controls);
        controls.Children.Add(CaptionButton("최소화", "M1,6 L11,6", delegate { WindowState = WindowState.Minimized; }));
        controls.Children.Add(CaptionButton("최대화 / 복원", "M1,1 L11,1 L11,11 L1,11 Z", delegate { WindowState = WindowState == WindowState.Maximized ? WindowState.Normal : WindowState.Maximized; }));
        var close = CaptionButton("닫기", "M1,1 L11,11 M11,1 L1,11", Close);
        close.MouseEnter += delegate { close.Background = new SolidColorBrush(Color.FromRgb(170, 38, 52)); };
        close.MouseLeave += delegate { close.Background = Brushes.Transparent; };
        controls.Children.Add(close);
    }
    Button CaptionButton(string name, string geometry, Action action)
    {
        var button = new Button { Style = (Style)FindResource("CaptionButton"), Content = Appearance.Glyph(geometry, 11), ToolTip = name };
        AutomationProperties.SetName(button, name); WindowChrome.SetIsHitTestVisibleInChrome(button, true);
        button.Click += delegate { action(); }; return button;
    }
    void ConfigureEmpty()
    {
        empty.Margin = new Thickness(0, Appearance.Number("emptyOffset") * 2, 0, 0);
        empty.Children.Add(new Image { Source = Appearance.BookMark(), Width = Appearance.Number("markSize"), Height = Appearance.Number("markSize"), Margin = new Thickness(0, 0, 0, 15) });
        empty.Children.Add(Centered("Sua View", "textPrimary", Appearance.Number("titleSize"), FontWeights.SemiBold, new Thickness(0, 0, 0, 15)));
        empty.Children.Add(Centered("만화책을 여기에 놓으세요", "textSecondary", Appearance.Number("subtitleSize"), FontWeights.Normal, new Thickness(0, 0, 0, 4)));
        empty.Children.Add(Centered("ZIP · CBZ · 이미지 폴더", "textMuted", Appearance.Number("detailSize"), FontWeights.Normal, new Thickness(0, 0, 0, 12)));
        var open = new Button { Content = "책 열기…", Style = (Style)FindResource("OpenButton"), Background = Appearance.Brush("button"), HorizontalAlignment = HorizontalAlignment.Center };
        open.Click += delegate { OpenDialog(); }; empty.Children.Add(open);
        var shortcut = Centered("Ctrl  O", "textMuted", 11, FontWeights.Normal, new Thickness(0, 5, 0, 0)); shortcut.FontFamily = new FontFamily("Consolas"); empty.Children.Add(shortcut);
    }
    TextBlock Centered(string text, string color, double size, FontWeight weight, Thickness margin)
    {
        return new TextBlock { Text = text, Foreground = Appearance.Brush(color), FontSize = size, FontWeight = weight, HorizontalAlignment = HorizontalAlignment.Center, Margin = margin };
    }
    Button HudButton(string name, string geometry, Action action)
    {
        var button = new Button { Style = (Style)FindResource("QuietButton"), Width = 28, Height = 30, Padding = new Thickness(5), Content = Appearance.Glyph(geometry, 16), ToolTip = name, Focusable = false };
        AutomationProperties.SetName(button, name); button.Click += delegate { action(); RevealHud(); }; return button;
    }
    void ConfigureHud()
    {
        hud.HorizontalAlignment = HorizontalAlignment.Center; hud.VerticalAlignment = VerticalAlignment.Bottom;
        hud.Margin = new Thickness(14, 0, 14, Appearance.Number("hudInset")); hud.Width = Appearance.Number("hudWidth");
        hud.Padding = new Thickness(12, 7, 12, 7); hud.CornerRadius = new CornerRadius(Appearance.Number("hudRadius"));
        hud.Background = new SolidColorBrush(Color.FromArgb(245, 39, 41, 46)); hud.BorderBrush = new SolidColorBrush(Color.FromArgb(28, 255, 255, 255)); hud.BorderThickness = new Thickness(1);
        hud.Effect = new DropShadowEffect { BlurRadius = 20, ShadowDepth = 3, Opacity = 0.24 };
        hud.Visibility = Visibility.Collapsed; surface.Children.Add(hud);
        var row = new Grid(); hud.Child = row;
        foreach (double width in new[] { 32.0, 36.0, -1.0, 82.0, 36.0, -2.0, 28.0 }) row.ColumnDefinitions.Add(new ColumnDefinition { Width = width == -1 ? new GridLength(1, GridUnitType.Star) : width == -2 ? GridLength.Auto : new GridLength(width) });
        Put(row, HudButton("다른 책 열기 · Ctrl+O", "M1,5 L7,5 L9,7 L20,7 L20,18 L1,18 Z M1,9 L20,9", OpenDialog), 0);
        backButton = HudButton("이전 페이지", "M12,2 L5,9 L12,16", Back); Put(row, backButton, 1);
        slider.Style = (Style)FindResource("PageSlider"); AutomationProperties.SetName(slider, "페이지 이동"); Put(row, slider, 2);
        slider.ValueChanged += delegate { if (!updatingSlider && reader != null) { ResetNavigation(); index = (int)Math.Round(slider.Value); Render(); } };
        var page = new Button { Content = pageNumber, Style = (Style)FindResource("QuietButton"), Padding = new Thickness(3), Focusable = false, ToolTip = "쪽 번호로 이동 · G" };
        pageNumber.FontFamily = new FontFamily("Consolas"); pageNumber.FontSize = 12; page.Click += delegate { Go(); }; Put(row, page, 3);
        nextButton = HudButton("다음 페이지", "M5,2 L12,9 L5,16", Next); Put(row, nextButton, 4);
        pageName.FontSize = 11; pageName.Foreground = Appearance.Brush("textSecondary"); pageName.TextTrimming = TextTrimming.CharacterEllipsis; pageName.MaxWidth = 150; pageName.VerticalAlignment = VerticalAlignment.Center; pageName.Margin = new Thickness(4, 0, 12, 0); Put(row, pageName, 5);
        Put(row, HudButton("읽기 옵션", "M2,8 L2.1,8 M9,8 L9.1,8 M16,8 L16.1,8", ShowOptions), 6);
        hud.MouseEnter += delegate { RevealHud(); }; hud.MouseLeave += delegate { hideHud.Stop(); hideHud.Start(); };
    }
    void Put(Grid grid, UIElement child, int column) { Grid.SetColumn(child, column); grid.Children.Add(child); }
    Window ThemedDialog(string title, FrameworkElement content, double width)
    {
        var dialog = new Window { Title = title, Owner = this, Width = width, SizeToContent = SizeToContent.Height, WindowStartupLocation = WindowStartupLocation.CenterOwner, Content = content, Background = Appearance.Brush("canvas"), Foreground = Appearance.Brush("textPrimary"), FontFamily = FontFamily, ResizeMode = ResizeMode.NoResize, ShowInTaskbar = false };
        dialog.SourceInitialized += delegate { int enabled = 1; DwmSetWindowAttribute(new WindowInteropHelper(dialog).Handle, 20, ref enabled, 4); };
        dialog.PreviewKeyDown += delegate(object sender, KeyEventArgs e) { if (e.Key == Key.Escape) dialog.Close(); };
        return dialog;
    }
    void UpdateHud()
    {
        if (reader == null) return;
        pageNumber.Text = (index + 1) + (shown == 2 ? "–" + (index + 2) : "") + " / " + reader.Count;
        pageName.Text = reader.Name(index); backButton.IsEnabled = index > 0; nextButton.IsEnabled = index + shown < reader.Count;
        updatingSlider = true; slider.Maximum = reader.Count - 1; slider.Value = index; updatingSlider = false;
        empty.Visibility = Visibility.Collapsed; status.Visibility = Visibility.Collapsed; scroll.Opacity = 1; RevealHud();
    }
    void RevealHud()
    {
        if (reader == null || closing) return;
        hud.Visibility = Visibility.Visible; hud.IsHitTestVisible = true;
        hud.BeginAnimation(OpacityProperty, new DoubleAnimation(1, TimeSpan.FromMilliseconds(120)));
        hideHud.Stop(); hideHud.Start();
    }
    void HideHud()
    {
        hideHud.Stop(); hud.IsHitTestVisible = false;
        hud.BeginAnimation(OpacityProperty, new DoubleAnimation(0, TimeSpan.FromMilliseconds(180)));
    }
    void ShowMessage(string text)
    {
        empty.Visibility = Visibility.Collapsed; status.Text = text; status.Visibility = Visibility.Visible; scroll.Opacity = 0.2;
    }
    ContextMenu BuildOptions()
    {
        var menu = new ContextMenu();
        AddOption(menu, "다른 책 열기…", "Ctrl+O", OpenDialog, null);
        AddOption(menu, "이미지 폴더 열기…", "Ctrl+Shift+O", OpenFolder, null);
        menu.Items.Add(new Separator());
        AddOption(menu, "두 페이지 보기", "D", delegate { dual = !dual; ResetNavigation(); Render(); }, dual);
        AddOption(menu, "오른쪽에서 왼쪽으로 읽기", "R", delegate { rtl = !rtl; Render(); }, rtl);
        menu.Items.Add(new Separator());
        AddOption(menu, "화면에 맞추기", "0", delegate { fitWidth = false; zoom = 1; Render(); }, !fitWidth && zoom == 1);
        AddOption(menu, "너비에 맞추기", "W", delegate { fitWidth = true; zoom = 1; Render(); }, fitWidth && zoom == 1);
        AddOption(menu, "확대", "+", delegate { Scale(1.2); }, null);
        AddOption(menu, "축소", "−", delegate { Scale(1 / 1.2); }, null);
        menu.Items.Add(new Separator());
        AddOption(menu, "쪽 번호로 이동…", "G", Go, null);
        AddOption(menu, "전체 화면", "F", Fullscreen, fullscreen);
        AddOption(menu, "단축키 도움말", "F1", Help, null);
        menu.Closed += delegate { options = null; RevealHud(); };
        return menu;
    }
    void AddOption(ContextMenu menu, string text, string key, Action action, bool? check)
    {
        var item = new MenuItem { Header = text, InputGestureText = key, IsCheckable = check.HasValue, IsChecked = check ?? false };
        item.Click += delegate { action(); }; menu.Items.Add(item);
    }
    void ShowOptions()
    {
        options = BuildOptions(); options.PlacementTarget = hud; options.Placement = System.Windows.Controls.Primitives.PlacementMode.Top; options.IsOpen = true;
    }
    void ConfigurePointer()
    {
        scroll.PreviewMouseWheel += delegate(object sender, MouseWheelEventArgs e) {
            RevealHud();
            if ((Keyboard.Modifiers & ModifierKeys.Control) != 0) { Scale(e.Delta > 0 ? 1.2 : 1 / 1.2); e.Handled = true; }
            else if ((Keyboard.Modifiers & ModifierKeys.Shift) != 0) { scroll.ScrollToHorizontalOffset(scroll.HorizontalOffset - e.Delta); e.Handled = true; }
            else if (scroll.ScrollableHeight < 1 && scroll.ScrollableWidth < 1 && reader != null) {
                if ((DateTime.UtcNow - lastWheel).TotalMilliseconds > 250) { if (e.Delta < 0) Next(); else Back(); lastWheel = DateTime.UtcNow; } e.Handled = true;
            }
        };
        scroll.MouseLeftButtonDown += delegate(object sender, MouseButtonEventArgs e) {
            Focus(); RevealHud(); doubleClicked = e.ClickCount == 2;
            if (doubleClicked) { Fullscreen(); e.Handled = true; return; }
            downPoint = e.GetPosition(scroll); dragging = false; dragHorizontal = scroll.HorizontalOffset; dragVertical = scroll.VerticalOffset; scroll.CaptureMouse();
        };
        scroll.MouseMove += delegate(object sender, MouseEventArgs e) {
            if (!downPoint.HasValue || e.LeftButton != MouseButtonState.Pressed) return;
            Point at = e.GetPosition(scroll); Vector delta = at - downPoint.Value;
            if (Math.Abs(delta.X) + Math.Abs(delta.Y) > 4) dragging = true;
            if (dragging) { scroll.ScrollToHorizontalOffset(dragHorizontal - delta.X); scroll.ScrollToVerticalOffset(dragVertical - delta.Y); }
        };
        scroll.MouseLeftButtonUp += delegate(object sender, MouseButtonEventArgs e) {
            bool click = downPoint.HasValue && !dragging && !doubleClicked; downPoint = null; scroll.ReleaseMouseCapture();
            if (!click || reader == null) return;
            double x = e.GetPosition(scroll).X;
            if (x < scroll.ActualWidth * 0.35) { if (rtl) Next(); else Back(); }
            else if (x > scroll.ActualWidth * 0.65) { if (rtl) Back(); else Next(); }
        };
        scroll.LostMouseCapture += delegate { downPoint = null; dragging = false; };
    }
}
