<#
.SYNOPSIS
Regenerates the raster icons from the logo geometry.

.DESCRIPTION
`plugins\frontend-web\assets\logo.svg` is the master. This script draws the
same shapes with System.Drawing (the numbers below mirror the SVG, on a
256-unit grid) and writes:

  packaging\windows\rah.ico            installer and shortcut icon (16-256 px)
  crates\rah-desktop\assets\icon.rgba  64x64 RGBA window icon for rah-desktop

The outputs are committed; run this only after changing the logo.

.EXAMPLE
.\scripts\make-icons.ps1
#>
[CmdletBinding()]
param()

$ErrorActionPreference = "Stop"
$repo = Split-Path -Parent $PSScriptRoot
Add-Type -AssemblyName System.Drawing

$blue = [System.Drawing.ColorTranslator]::FromHtml("#2f5bd3")

function New-LogoBitmap([int]$size) {
    $bitmap = New-Object System.Drawing.Bitmap $size, $size, ([System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
    $g = [System.Drawing.Graphics]::FromImage($bitmap)
    $g.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::AntiAlias
    $g.PixelOffsetMode = [System.Drawing.Drawing2D.PixelOffsetMode]::HighQuality
    $g.Clear([System.Drawing.Color]::Transparent)
    $g.ScaleTransform($size / 256.0, $size / 256.0)

    # Rounded square, radius 56.
    $path = New-Object System.Drawing.Drawing2D.GraphicsPath
    $d = 112
    $path.AddArc(0, 0, $d, $d, 180, 90)
    $path.AddArc(256 - $d, 0, $d, $d, 270, 90)
    $path.AddArc(256 - $d, 256 - $d, $d, $d, 0, 90)
    $path.AddArc(0, 256 - $d, $d, $d, 90, 90)
    $path.CloseFigure()
    $g.FillPath((New-Object System.Drawing.SolidBrush $blue), $path)

    $white = [System.Drawing.Color]::White
    $pen = New-Object System.Drawing.Pen $white, 10
    $pen.StartCap = [System.Drawing.Drawing2D.LineCap]::Round
    $pen.EndCap = [System.Drawing.Drawing2D.LineCap]::Round
    $brush = New-Object System.Drawing.SolidBrush $white
    foreach ($p in @(@(68, 68), @(188, 68), @(68, 188), @(188, 188))) {
        $g.DrawLine($pen, 128, 128, $p[0], $p[1])
        $g.FillEllipse($brush, $p[0] - 17, $p[1] - 17, 34, 34)
    }
    $g.FillEllipse($brush, 128 - 36, 128 - 36, 72, 72)
    $g.FillEllipse((New-Object System.Drawing.SolidBrush $blue), 128 - 16, 128 - 16, 32, 32)
    $g.Dispose()
    return $bitmap
}

function Get-Png([int]$size) {
    $bitmap = New-LogoBitmap $size
    $stream = New-Object System.IO.MemoryStream
    $bitmap.Save($stream, [System.Drawing.Imaging.ImageFormat]::Png)
    $bitmap.Dispose()
    return , $stream.ToArray()
}

# ICO with PNG-compressed entries (Windows Vista and later).
$sizes = 16, 24, 32, 48, 64, 128, 256
$pngs = foreach ($s in $sizes) { , (Get-Png $s) }
$out = New-Object System.IO.MemoryStream
$w = New-Object System.IO.BinaryWriter $out
$w.Write([uint16]0); $w.Write([uint16]1); $w.Write([uint16]$sizes.Count)
$offset = 6 + 16 * $sizes.Count
for ($i = 0; $i -lt $sizes.Count; $i++) {
    $dim = if ($sizes[$i] -ge 256) { 0 } else { $sizes[$i] }
    $w.Write([byte]$dim); $w.Write([byte]$dim); $w.Write([byte]0); $w.Write([byte]0)
    $w.Write([uint16]1); $w.Write([uint16]32)
    $w.Write([uint32]$pngs[$i].Length); $w.Write([uint32]$offset)
    $offset += $pngs[$i].Length
}
foreach ($png in $pngs) { $w.Write($png) }
$w.Flush()
$icoPath = Join-Path $repo "packaging\windows\rah.ico"
[System.IO.File]::WriteAllBytes($icoPath, $out.ToArray())
Write-Host "Wrote $icoPath"

# Raw RGBA for tao's Icon::from_rgba.
$size = 64
$bitmap = New-LogoBitmap $size
$rgba = New-Object byte[] ($size * $size * 4)
for ($y = 0; $y -lt $size; $y++) {
    for ($x = 0; $x -lt $size; $x++) {
        $c = $bitmap.GetPixel($x, $y)
        $i = ($y * $size + $x) * 4
        $rgba[$i] = $c.R; $rgba[$i + 1] = $c.G; $rgba[$i + 2] = $c.B; $rgba[$i + 3] = $c.A
    }
}
$bitmap.Dispose()
$assets = Join-Path $repo "crates\rah-desktop\assets"
New-Item -ItemType Directory -Force -Path $assets | Out-Null
[System.IO.File]::WriteAllBytes((Join-Path $assets "icon.rgba"), $rgba)
Write-Host "Wrote $(Join-Path $assets 'icon.rgba')"
