#!/usr/bin/swift
// assets/disk-lens.svg → macOS iconset. 아이콘 원본은 직접 다운로드판(disk-lens)과 같은 파일이다.
// 사용법 (disk-lens-appstore 폴더에서): swift scripts/make-icon.swift <iconset 폴더>
import AppKit

guard let svg = NSImage(contentsOf: URL(fileURLWithPath: "assets/disk-lens.svg")) else {
    fatalError("assets/disk-lens.svg 를 읽지 못했습니다. disk-lens-appstore 폴더에서 실행하세요.")
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

let iconset = URL(fileURLWithPath: CommandLine.arguments.dropFirst().first ?? "dist/DiskLens.iconset")
try FileManager.default.createDirectory(at: iconset, withIntermediateDirectories: true)
for base in [16, 32, 128, 256, 512] {
    try png(base).write(to: iconset.appendingPathComponent("icon_\(base)x\(base).png"))
    try png(base * 2).write(to: iconset.appendingPathComponent("icon_\(base)x\(base)@2x.png"))
}
