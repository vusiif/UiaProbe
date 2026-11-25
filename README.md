# UiaProbe

A lightweight, read-only, non-intrusive Windows GUI structure perception CLI tool written in Rust.

`UiaProbe` (`uiaprobe.exe`) traverses Windows UI Automation (UIA) control trees and Chromium DOMs, filters interactive elements, normalizes multi-monitor/DPI physical coordinates, and outputs structured JSON to `stdout`.
