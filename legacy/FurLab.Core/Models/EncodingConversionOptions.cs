namespace FurLab.Core.Models;

/// <summary>
/// Defines options for encoding conversion operations.
/// </summary>
public record EncodingConversionOptions
{
    /// <summary>
    /// Gets or sets the glob pattern for matching files (e.g., "**/*.cs").
    /// </summary>
    public string Pattern { get; init; } = string.Empty;

    /// <summary>
    /// Gets or sets the source encoding. If null, auto-detection will be used.
    /// </summary>
    public string? SourceEncoding { get; init; }

    /// <summary>
    /// Gets or sets the target encoding.
    /// </summary>
    public string TargetEncoding { get; init; } = "UTF-8";

    /// <summary>
    /// Gets or sets the output directory. If null, files are converted in-place.
    /// </summary>
    public string? OutputDirectory { get; init; }

    /// <summary>
    /// Gets or sets whether to create backup files (.bak) before conversion.
    /// </summary>
    public bool CreateBackup { get; init; }

    /// <summary>
    /// Gets or sets whether to filter only known text file extensions.
    /// </summary>
    public bool TextOnly { get; init; }

    /// <summary>
    /// Gets or sets the minimum confidence threshold for auto-detection (0.0 to 1.0).
    /// </summary>
    public double ConfidenceThreshold { get; init; } = 0.8;

    /// <summary>
    /// Gets or sets whether to force conversion even when confidence is below threshold.
    /// </summary>
    public bool Force { get; init; }

    /// <summary>
    /// Gets or sets patterns to exclude (e.g., "node_modules/**").
    /// </summary>
    public IReadOnlyList<string> ExcludePatterns { get; init; } = [];
}
