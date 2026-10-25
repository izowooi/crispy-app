#!/usr/bin/swift
import AppKit
import Foundation

// The editable SVG is the only artwork source. This small renderer supports its
// deliberately flat rect/circle/line vocabulary without external dependencies.
struct Element { let kind: String; let attributes: [String: String] }
final class SVG: NSObject, XMLParserDelegate {
    var elements: [Element] = []
    func parser(_ parser: XMLParser, didStartElement name: String, namespaceURI: String?, qualifiedName: String?, attributes: [String: String]) {
        if ["rect", "circle", "line"].contains(name) { elements.append(Element(kind: name, attributes: attributes)) }
    }
}
func color(_ value: String) -> NSColor? {
    guard value.hasPrefix("#"), let rgb = UInt32(value.dropFirst(), radix: 16) else { return nil }
    return NSColor(srgbRed: CGFloat((rgb >> 16) & 255)/255, green: CGFloat((rgb >> 8) & 255)/255, blue: CGFloat(rgb & 255)/255, alpha: 1)
}
let output = CommandLine.arguments.dropFirst().first ?? "dist/DiskLens.iconset"
let source = CommandLine.arguments.dropFirst(2).first ?? "assets/disk-lens.svg"
let svg = SVG()
let parser = XMLParser(data: try Data(contentsOf: URL(fileURLWithPath: source)))
parser.delegate = svg
guard parser.parse(), !svg.elements.isEmpty else { fatalError("Invalid SVG: \(source)") }
try FileManager.default.createDirectory(atPath: output, withIntermediateDirectories: true)
func draw(size: Int, name: String) throws {
    guard let bitmap = NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: size, pixelsHigh: size, bitsPerSample: 8, samplesPerPixel: 4, hasAlpha: true, isPlanar: false, colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0), let context = NSGraphicsContext(bitmapImageRep: bitmap) else { fatalError("Cannot create icon bitmap") }
    NSGraphicsContext.saveGraphicsState()
    NSGraphicsContext.current = context
    context.cgContext.scaleBy(x: CGFloat(size)/1024, y: CGFloat(size)/1024)
    context.cgContext.translateBy(x: 0, y: 1024)
    context.cgContext.scaleBy(x: 1, y: -1)
    for element in svg.elements {
        let a = element.attributes
        func n(_ key: String) -> CGFloat { CGFloat(Double(a[key] ?? "0") ?? 0) }
        let path: NSBezierPath
        switch element.kind {
        case "rect": path = NSBezierPath(roundedRect: NSRect(x: n("x"), y: n("y"), width: n("width"), height: n("height")), xRadius: n("rx"), yRadius: n("rx"))
        case "circle": path = NSBezierPath(ovalIn: NSRect(x: n("cx")-n("r"), y: n("cy")-n("r"), width: n("r")*2, height: n("r")*2))
        default:
            path = NSBezierPath(); path.move(to: NSPoint(x: n("x1"), y: n("y1"))); path.line(to: NSPoint(x: n("x2"), y: n("y2")))
        }
        if let fill = color(a["fill"] ?? "none") { fill.setFill(); path.fill() }
        if let stroke = color(a["stroke"] ?? "none") {
            stroke.setStroke(); path.lineWidth = n("stroke-width")
            if a["stroke-linecap"] == "round" { path.lineCapStyle = .round }
            path.stroke()
        }
    }
    NSGraphicsContext.restoreGraphicsState()
    guard let data = bitmap.representation(using: .png, properties: [:]) else { fatalError("PNG encoding failed") }
    try data.write(to: URL(fileURLWithPath: output).appendingPathComponent(name))
}
for points in [16, 32, 128, 256, 512] {
    try draw(size: points, name: "icon_\(points)x\(points).png")
    try draw(size: points*2, name: "icon_\(points)x\(points)@2x.png")
}
