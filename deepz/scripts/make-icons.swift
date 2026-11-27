// assets/icon.svg → mac/AppIcon.icns, windows/icon.ico, assets/icon.png
// 사용: swift scripts/make-icons.swift   (deepz 폴더에서, macOS 기본 도구만 사용)
import AppKit

let root = URL(fileURLWithPath: FileManager.default.currentDirectoryPath)
guard let svg = NSImage(contentsOf: root.appendingPathComponent("assets/icon.svg")) else {
    fatalError("assets/icon.svg 를 읽지 못했습니다. deepz 폴더에서 실행하세요.")
}

func png(_ size: Int) -> Data {
    let rep = NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: size, pixelsHigh: size, bitsPerSample: 8,
                               samplesPerPixel: 4, hasAlpha: true, isPlanar: false,
                               colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0)!
    rep.size = NSSize(width: size, height: size)
    NSGraphicsContext.saveGraphicsState()
    NSGraphicsContext.current = NSGraphicsContext(bitmapImageRep: rep)
    NSGraphicsContext.current?.imageInterpolation = .high
    svg.draw(in: NSRect(x: 0, y: 0, width: size, height: size))
    NSGraphicsContext.restoreGraphicsState()
    return rep.representation(using: .png, properties: [:])!
}

// macOS: iconset → icns
let iconset = FileManager.default.temporaryDirectory.appendingPathComponent("DeepZ-\(getpid()).iconset")
try FileManager.default.createDirectory(at: iconset, withIntermediateDirectories: true)
for base in [16, 32, 128, 256, 512] {
    try png(base).write(to: iconset.appendingPathComponent("icon_\(base)x\(base).png"))
    try png(base * 2).write(to: iconset.appendingPathComponent("icon_\(base)x\(base)@2x.png"))
}
let iconutil = Process()
iconutil.executableURL = URL(fileURLWithPath: "/usr/bin/iconutil")
iconutil.arguments = ["-c", "icns", iconset.path, "-o", root.appendingPathComponent("mac/AppIcon.icns").path]
try iconutil.run()
iconutil.waitUntilExit()
try? FileManager.default.removeItem(at: iconset)

// Windows: 크기별 PNG 를 담은 ico
let sizes = [16, 20, 24, 32, 40, 48, 64, 256]
let images = sizes.map(png)
func le<T: FixedWidthInteger>(_ value: T) -> Data { withUnsafeBytes(of: value.littleEndian) { Data($0) } }
var ico = le(UInt16(0)) + le(UInt16(1)) + le(UInt16(sizes.count))
var offset = 6 + 16 * sizes.count
for (size, image) in zip(sizes, images) {
    let side = UInt8(size == 256 ? 0 : size)
    ico += Data([side, side, 0, 0]) + le(UInt16(1)) + le(UInt16(32)) + le(UInt32(image.count)) + le(UInt32(offset))
    offset += image.count
}
images.forEach { ico += $0 }
try ico.write(to: root.appendingPathComponent("windows/icon.ico"))

// README 용
try png(512).write(to: root.appendingPathComponent("assets/icon.png"))
print("만들었습니다: mac/AppIcon.icns, windows/icon.ico, assets/icon.png")
