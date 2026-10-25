using System;
using System.Collections;
using System.Collections.Generic;
using System.Globalization;
using System.IO;
using System.Linq;
using System.Reflection;
using System.Web.Script.Serialization;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Markup;
using System.Windows.Media;
using System.Windows.Shapes;

// Palette, geometry and dimensions are shared with the AppKit adapter.
internal static class Appearance
{
    static readonly Dictionary<string, object> values;
    static Appearance()
    {
        using (var stream = Assembly.GetExecutingAssembly().GetManifestResourceStream("SuaView.ReaderTheme.json"))
        using (var reader = new StreamReader(stream)) values = new JavaScriptSerializer().Deserialize<Dictionary<string, object>>(reader.ReadToEnd());
    }
    internal static double Number(string name) { return Convert.ToDouble(values[name], CultureInfo.InvariantCulture); }
    internal static SolidColorBrush Brush(string name)
    {
        var brush = new SolidColorBrush((Color)ColorConverter.ConvertFromString((string)values[name])); brush.Freeze(); return brush;
    }
    internal static ResourceDictionary Styles()
    {
        using (var stream = Assembly.GetExecutingAssembly().GetManifestResourceStream("SuaView.ReaderStyles.xaml")) return (ResourceDictionary)XamlReader.Load(stream);
    }
    internal static DrawingImage BookMark()
    {
        var group = new DrawingGroup();
        group.Children.Add(new GeometryDrawing(Brushes.Transparent, null, new RectangleGeometry(new Rect(0, 0, 1, 1))));
        foreach (string side in new[] { "markLeft", "markRight" }) {
            var points = ((IEnumerable)values[side]).Cast<object>().Select(p => {
                var pair = ((IEnumerable)p).Cast<object>().ToArray(); return new Point(Convert.ToDouble(pair[0]), Convert.ToDouble(pair[1]));
            }).ToArray();
            var geometry = new StreamGeometry();
            using (var context = geometry.Open()) { context.BeginFigure(points[0], true, true); context.PolyLineTo(points.Skip(1).ToArray(), true, false); }
            group.Children.Add(new GeometryDrawing(Brush("bookMark"), null, geometry));
        }
        var image = new DrawingImage(group); image.Freeze(); return image;
    }
    internal static FrameworkElement Glyph(string data, double size)
    {
        return new Viewbox { Width = size, Height = size, Stretch = Stretch.Uniform,
            Child = new System.Windows.Shapes.Path { Data = Geometry.Parse(data), Stroke = Brush("textPrimary"), StrokeThickness = 1.5,
                StrokeStartLineCap = PenLineCap.Round, StrokeEndLineCap = PenLineCap.Round, StrokeLineJoin = PenLineJoin.Round } };
    }
}
