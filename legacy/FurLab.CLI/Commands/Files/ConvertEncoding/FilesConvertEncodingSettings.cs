using System.ComponentModel;

using Spectre.Console.Cli;

namespace FurLab.CLI.Commands.Files.ConvertEncoding;

/// <summary>
/// Settings for the file convert-encoding command.
/// </summary>
public sealed class FilesConvertEncodingSettings : CommandSettings
{
    /// <summary>
    /// Gets the input glob pattern (e.g., "**/*.cs", "src/**/*.txt").
    /// </summary>
    [CommandOption("-i|--input")]
    [Description("Input glob pattern (e.g., **/*.cs, src/**/*.txt).")]
    public string Input { get; init; } = string.Empty;

    /// <summary>
    /// Gets the source encoding. If not specified, auto-detection is used.
    /// </summary>
    [CommandOption("--from")]
    [Description("Source encoding (e.g., UTF-8, Latin1, Windows-1252). If omitted, auto-detection is used.")]
    public string? SourceEncoding { get; init; }

    /// <summary>
    /// Gets the target encoding.
    /// </summary>
    [CommandOption("--to")]
    [Description("Target encoding (e.g., UTF-8, UTF-8-BOM, UTF-16, Latin1, Windows-1252).")]
    public string TargetEncoding { get; init; } = "UTF-8";

    /// <summary>
    /// Gets the output directory. If not specified, files are converted in-place.
    /// </summary>
    [CommandOption("-o|--output")]
    [Description("Output directory. If omitted, files are converted in-place.")]
    public string? OutputDirectory { get; init; }

    /// <summary>
    /// Gets whether to create backup files (.bak) before conversion.
    /// </summary>
    [CommandOption("--backup")]
    [Description("Create backup files (.bak) before conversion.")]
    public bool CreateBackup { get; init; }

    /// <summary>
    /// Gets whether to filter only known text file extensions.
    /// </summary>
    [CommandOption("--text-only")]
    [Description("Filter only known text file extensions.")]
    public bool TextOnly { get; init; }

    /// <summary>
    /// Gets the minimum confidence threshold for auto-detection (0.0 to 1.0).
    /// </summary>
    [CommandOption("--confidence")]
    [Description("Minimum confidence threshold for auto-detection (0.0 to 1.0). Default: 0.8")]
    public double ConfidenceThreshold { get; init; } = 0.8;

    /// <summary>
    /// Gets whether to force conversion even with low confidence.
    /// </summary>
    [CommandOption("--force")]
    [Description("Force conversion even when confidence is below threshold.")]
    public bool Force { get; init; }

    /// <summary>
    /// Gets patterns to exclude (comma-separated).
    /// </summary>
    [CommandOption("--exclude")]
    [Description("Patterns to exclude (comma-separated, e.g., \"node_modules/**,bin/**\").")]
    public string? ExcludePatterns { get; init; }
}
