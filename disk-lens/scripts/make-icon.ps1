param([Parameter(Mandatory=$true)][string]$Output)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
[xml]$svg = Get-Content -Raw (Join-Path $PSScriptRoot '../assets/disk-lens.svg')
$images = @()
foreach ($size in @(16, 32, 48, 64, 128, 256)) {
    $bitmap = New-Object System.Drawing.Bitmap $size, $size
    $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
    $graphics.SmoothingMode = 'AntiAlias'
    $graphics.ScaleTransform(($size / 1024.0), ($size / 1024.0))
    foreach ($element in $svg.svg.ChildNodes) {
        $path = New-Object System.Drawing.Drawing2D.GraphicsPath
        switch ($element.LocalName) {
            'rect' {
                $x = [single]$element.x; $y = [single]$element.y
                $w = [single]$element.width; $h = [single]$element.height
                $r = [single]$element.rx; $d = 2 * $r
                if ($r -gt 0) {
                    $path.AddArc($x, $y, $d, $d, 180, 90)
                    $path.AddArc(($x+$w-$d), $y, $d, $d, 270, 90)
                    $path.AddArc(($x+$w-$d), ($y+$h-$d), $d, $d, 0, 90)
                    $path.AddArc($x, ($y+$h-$d), $d, $d, 90, 90)
                    $path.CloseFigure()
                } else { $path.AddRectangle([System.Drawing.RectangleF]::new($x,$y,$w,$h)) }
            }
            'circle' {
                $r = [single]$element.r
                $path.AddEllipse(([single]$element.cx-$r), ([single]$element.cy-$r), (2*$r), (2*$r))
            }
            'line' { $path.AddLine([single]$element.x1,[single]$element.y1,[single]$element.x2,[single]$element.y2) }
            default { $path.Dispose(); continue }
        }
        if ($element.fill -and $element.fill -ne 'none') {
            $brush = New-Object System.Drawing.SolidBrush ([System.Drawing.ColorTranslator]::FromHtml($element.fill))
            $graphics.FillPath($brush,$path); $brush.Dispose()
        }
        if ($element.stroke) {
            $pen = New-Object System.Drawing.Pen ([System.Drawing.ColorTranslator]::FromHtml($element.stroke)), ([single]$element.'stroke-width')
            if ($element.'stroke-linecap' -eq 'round') { $pen.StartCap='Round'; $pen.EndCap='Round' }
            $graphics.DrawPath($pen,$path); $pen.Dispose()
        }
        $path.Dispose()
    }
    $stream = New-Object System.IO.MemoryStream
    $bitmap.Save($stream,[System.Drawing.Imaging.ImageFormat]::Png)
    if ($size -eq 256) { $bitmap.Save("$Output.png",[System.Drawing.Imaging.ImageFormat]::Png) }
    $images += ,($stream.ToArray())
    $stream.Dispose(); $graphics.Dispose(); $bitmap.Dispose()
}
$file = [System.IO.File]::Create($Output)
$writer = New-Object System.IO.BinaryWriter $file
try {
    $writer.Write([uint16]0); $writer.Write([uint16]1); $writer.Write([uint16]$images.Count)
    $offset = 6 + 16 * $images.Count
    $sizes = @(16,32,48,64,128,256)
    for ($i=0; $i -lt $images.Count; $i++) {
        $dimension = if ($sizes[$i] -eq 256) { 0 } else { $sizes[$i] }
        $writer.Write([byte]$dimension); $writer.Write([byte]$dimension)
        $writer.Write([byte]0); $writer.Write([byte]0)
        $writer.Write([uint16]1); $writer.Write([uint16]32)
        $writer.Write([uint32]$images[$i].Length); $writer.Write([uint32]$offset)
        $offset += $images[$i].Length
    }
    foreach ($data in $images) { $writer.Write([byte[]]$data) }
} finally { $writer.Dispose(); $file.Dispose() }
