<#
.SYNOPSIS
Regenerates the raster icons from the logo geometry.

.DESCRIPTION
`plugins\frontend-web\assets\logo.svg` is the master. This script draws the
SVG rect/polygon shapes with System.Drawing on a
256-unit grid and writes:

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

# Keep the SVG as the single source of geometry and palette. This intentionally
# supports only the rect/polygon vocabulary used by the logo; fail on new shapes.
[xml]$logo = Get-Content -Raw (Join-Path $repo 'plugins\frontend-web\assets\logo.svg')

function New-LogoBitmap([int]$size) {
    $scale = 4
    $large = New-Object System.Drawing.Bitmap ($size * $scale), ($size * $scale), ([System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
    $g = [System.Drawing.Graphics]::FromImage($large)
    try {
        $g.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::AntiAlias
        $g.Clear([System.Drawing.Color]::Transparent)
        $g.ScaleTransform($size * $scale / 256.0, $size * $scale / 256.0)
        foreach ($shape in $logo.DocumentElement.ChildNodes) {
            if ($shape.LocalName -in @('title', 'desc', '#comment')) { continue }
            $brush = New-Object System.Drawing.SolidBrush ([System.Drawing.ColorTranslator]::FromHtml($shape.GetAttribute('fill')))
            try {
                switch ($shape.LocalName) {
                    'rect' {
                        $x = [float]$shape.GetAttribute('x')
                        $y = [float]$shape.GetAttribute('y')
                        $width = [float]$shape.GetAttribute('width')
                        $height = [float]$shape.GetAttribute('height')
                        $diameter = 2 * [float]$shape.GetAttribute('rx')
                        if ($diameter -eq 0) {
                            $g.FillRectangle($brush, $x, $y, $width, $height)
                        } else {
                            $path = New-Object System.Drawing.Drawing2D.GraphicsPath
                            try {
                                $path.AddArc($x, $y, $diameter, $diameter, 180, 90)
                                $path.AddArc($x + $width - $diameter, $y, $diameter, $diameter, 270, 90)
                                $path.AddArc($x + $width - $diameter, $y + $height - $diameter, $diameter, $diameter, 0, 90)
                                $path.AddArc($x, $y + $height - $diameter, $diameter, $diameter, 90, 90)
                                $path.CloseFigure()
                                $g.FillPath($brush, $path)
                            } finally { $path.Dispose() }
                        }
                    }
                    'polygon' {
                        [System.Drawing.PointF[]]$points = foreach ($pair in ($shape.GetAttribute('points') -split '\s+')) {
                            $xy = $pair -split ','
                            New-Object System.Drawing.PointF ([float]$xy[0]), ([float]$xy[1])
                        }
                        $g.FillPolygon($brush, $points)
                    }
                    default { throw "Unsupported logo shape: $($shape.LocalName)" }
                }
            } finally { $brush.Dispose() }
        }
        $bitmap = New-Object System.Drawing.Bitmap $size, $size, ([System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
        $outputGraphics = [System.Drawing.Graphics]::FromImage($bitmap)
        try {
            $outputGraphics.CompositingMode = [System.Drawing.Drawing2D.CompositingMode]::SourceCopy
            $outputGraphics.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
            $outputGraphics.PixelOffsetMode = [System.Drawing.Drawing2D.PixelOffsetMode]::HighQuality
            $outputGraphics.DrawImage($large, 0, 0, $size, $size)
        } finally { $outputGraphics.Dispose() }
        return $bitmap
    } finally {
        $g.Dispose()
        $large.Dispose()
    }
}
function Get-Png([int]$size) {
    $bitmap = New-LogoBitmap $size
    $stream = New-Object System.IO.MemoryStream
    $bitmap.Save($stream, [System.Drawing.Imaging.ImageFormat]::Png)
    $bitmap.Dispose()
    $bytes = $stream.ToArray()
    $stream.Dispose()
    return , $bytes
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
$w.Dispose()
$out.Dispose()
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
