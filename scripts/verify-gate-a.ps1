param(
    [string]$ProcessName = "epet-desktop",
    [string]$OutputPath,
    [switch]$ExerciseClick
)

$ErrorActionPreference = "Stop"

Add-Type -AssemblyName System.Windows.Forms
if (-not ("GateANative" -as [type])) {
    Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;
using System.Text;

public static class GateANative {
    public delegate bool EnumWindowsProc(IntPtr hwnd, IntPtr parameter);

    [StructLayout(LayoutKind.Sequential)]
    public struct Rect {
        public int Left;
        public int Top;
        public int Right;
        public int Bottom;
    }

    [StructLayout(LayoutKind.Sequential)]
    public struct Point {
        public int X;
        public int Y;
    }

    [DllImport("user32.dll")]
    public static extern bool EnumWindows(EnumWindowsProc callback, IntPtr parameter);

    [DllImport("user32.dll")]
    public static extern uint GetWindowThreadProcessId(IntPtr hwnd, out uint processId);

    [DllImport("user32.dll", EntryPoint = "GetWindowLongPtrW")]
    public static extern IntPtr GetWindowLongPtr(IntPtr hwnd, int index);

    [DllImport("user32.dll")]
    public static extern bool GetWindowRect(IntPtr hwnd, out Rect rect);

    [DllImport("user32.dll")]
    public static extern bool IsWindowVisible(IntPtr hwnd);

    [DllImport("user32.dll")]
    public static extern IntPtr SendMessageW(IntPtr hwnd, uint message, IntPtr wParam, IntPtr lParam);

    [DllImport("user32.dll")]
    public static extern IntPtr GetForegroundWindow();

    [DllImport("user32.dll")]
    public static extern uint GetDpiForWindow(IntPtr hwnd);

    [DllImport("user32.dll")]
    public static extern bool GetCursorPos(out Point point);

    [DllImport("user32.dll")]
    public static extern bool SetCursorPos(int x, int y);

    [DllImport("user32.dll")]
    public static extern void mouse_event(uint flags, uint dx, uint dy, uint data, UIntPtr extra);

    [DllImport("user32.dll", CharSet = CharSet.Unicode)]
    public static extern int GetWindowTextW(IntPtr hwnd, StringBuilder text, int count);
}
"@
}

function New-HitTestParameter {
    param([int]$X, [int]$Y)
    $packed = (($Y -band 0xffff) -shl 16) -bor ($X -band 0xffff)
    return [IntPtr]::new([int64][uint32]$packed)
}

$process = Get-Process -Name $ProcessName -ErrorAction Stop | Select-Object -First 1
$windows = [System.Collections.Generic.List[object]]::new()
$callback = [GateANative+EnumWindowsProc]{
    param([IntPtr]$handle, [IntPtr]$parameter)
    $windowProcessId = [uint32]0
    [void][GateANative]::GetWindowThreadProcessId($handle, [ref]$windowProcessId)
    if ($windowProcessId -ne $process.Id) {
        return $true
    }

    $rect = [GateANative+Rect]::new()
    [void][GateANative]::GetWindowRect($handle, [ref]$rect)
    $titleBuffer = [Text.StringBuilder]::new(256)
    [void][GateANative]::GetWindowTextW($handle, $titleBuffer, $titleBuffer.Capacity)
    $extendedStyle = [GateANative]::GetWindowLongPtr($handle, -20).ToInt64()
    $windows.Add([pscustomobject]@{
        Handle = $handle
        HandleHex = "0x{0:X}" -f $handle.ToInt64()
        Title = $titleBuffer.ToString()
        Visible = [GateANative]::IsWindowVisible($handle)
        Left = $rect.Left
        Top = $rect.Top
        Right = $rect.Right
        Bottom = $rect.Bottom
        Width = $rect.Right - $rect.Left
        Height = $rect.Bottom - $rect.Top
        ExtendedStyle = $extendedStyle
        ToolWindow = ($extendedStyle -band 0x00000080) -ne 0
        NoActivate = ($extendedStyle -band 0x08000000) -ne 0
        TopMost = ($extendedStyle -band 0x00000008) -ne 0
        Dpi = [GateANative]::GetDpiForWindow($handle)
    })
    return $true
}
[void][GateANative]::EnumWindows($callback, [IntPtr]::Zero)

$pet = $windows |
    Where-Object { $_.ToolWindow -and $_.NoActivate -and $_.Width -gt 0 -and $_.Height -gt 0 } |
    Select-Object -First 1
if (-not $pet) {
    throw "No pet window with WS_EX_TOOLWINDOW and WS_EX_NOACTIVATE was found."
}

$cornerX = $pet.Left + 2
$cornerY = $pet.Top + 2
$centerX = $pet.Left + [Math]::Floor($pet.Width / 2)
$centerY = $pet.Top + [Math]::Floor($pet.Height / 2)
$foregroundBefore = [GateANative]::GetForegroundWindow()
$cornerResult = [GateANative]::SendMessageW(
    $pet.Handle,
    0x0084,
    [IntPtr]::Zero,
    (New-HitTestParameter -X $cornerX -Y $cornerY)
).ToInt64()
$centerResult = [GateANative]::SendMessageW(
    $pet.Handle,
    0x0084,
    [IntPtr]::Zero,
    (New-HitTestParameter -X $centerX -Y $centerY)
).ToInt64()
$foregroundAfter = [GateANative]::GetForegroundWindow()
$realClickFocusUnchanged = $null
if ($ExerciseClick) {
    $cursor = [GateANative+Point]::new()
    [void][GateANative]::GetCursorPos([ref]$cursor)
    $clickForegroundBefore = [GateANative]::GetForegroundWindow()
    [void][GateANative]::SetCursorPos($centerX, $centerY)
    Start-Sleep -Milliseconds 100
    [GateANative]::mouse_event(0x0002, 0, 0, 0, [UIntPtr]::Zero)
    [GateANative]::mouse_event(0x0004, 0, 0, 0, [UIntPtr]::Zero)
    Start-Sleep -Milliseconds 120
    $clickForegroundAfter = [GateANative]::GetForegroundWindow()
    [void][GateANative]::SetCursorPos($cursor.X, $cursor.Y)
    $realClickFocusUnchanged = $clickForegroundBefore -eq $clickForegroundAfter
}

$screens = [System.Windows.Forms.Screen]::AllScreens | ForEach-Object {
    [pscustomobject]@{
        DeviceName = $_.DeviceName
        Primary = $_.Primary
        Bounds = [pscustomobject]@{
            X = $_.Bounds.X
            Y = $_.Bounds.Y
            Width = $_.Bounds.Width
            Height = $_.Bounds.Height
        }
        WorkingArea = [pscustomobject]@{
            X = $_.WorkingArea.X
            Y = $_.WorkingArea.Y
            Width = $_.WorkingArea.Width
            Height = $_.WorkingArea.Height
        }
    }
}

$result = [pscustomobject]@{
    Timestamp = (Get-Date).ToUniversalTime().ToString("o")
    ProcessId = $process.Id
    ProcessVersion = $process.ProductVersion
    OsVersion = [Environment]::OSVersion.VersionString
    Screens = @($screens)
    PetWindow = $pet
    Checks = [pscustomobject]@{
        TransparentCornerReturnsHtTransparent = $cornerResult -eq -1
        OpaqueCenterIsInteractive = $centerResult -ne -1
        ForegroundUnchangedDuringHitTest = $foregroundBefore -eq $foregroundAfter
        ForegroundUnchangedDuringRealClick = $realClickFocusUnchanged
        ToolWindow = $pet.ToolWindow
        NoActivate = $pet.NoActivate
        HiddenFromAltTabByToolWindowStyle = $pet.ToolWindow
        EntireWindowInsideAWorkingArea = @($screens | Where-Object {
            $pet.Left -ge $_.WorkingArea.X -and
            $pet.Top -ge $_.WorkingArea.Y -and
            $pet.Right -le ($_.WorkingArea.X + $_.WorkingArea.Width) -and
            $pet.Bottom -le ($_.WorkingArea.Y + $_.WorkingArea.Height)
        }).Count -gt 0
    }
    RawHitTest = [pscustomobject]@{
        Corner = $cornerResult
        Center = $centerResult
    }
    ManualEvidenceStillRequired = @(
        "Physical drag, scale, and restart at 125%, 150%, and 200% DPI",
        "Physical dual-monitor, negative-origin, and hot-plug coverage",
        "Real mouse clicks in transparent and opaque regions",
        "Real Alt-Tab order and keyboard-focus switching",
        "Complete hidden, drag, and autonomous-movement matrix",
        "8-hour Windows 11 and 2-hour Windows 10 soak tests"
    )
}

$json = $result | ConvertTo-Json -Depth 8
if ($OutputPath) {
    $resolvedOutput = $ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($OutputPath)
    [IO.File]::WriteAllText($resolvedOutput, $json, [Text.UTF8Encoding]::new($false))
}
$json
