namespace FurLab.Core.Models;

/// <summary>
/// Information about a processed file (converted or skipped).
/// </summary>
public record ProcessedFileInfo
{
    /// <summary>
    /// Gets or sets the original file path.
    /// </summary>
    public string OriginalPath { get; init; } = string.Empty;

    /// <summary>
    /// Gets or sets the output file path.
    /// </summary>
    public string OutputPath { get; init; } = string.Empty;

    /// <summary>
    /// Gets or sets the detected source encoding.
    /// </summary>
    public string SourceEncoding { get; init; } = string.Empty;

    /// <summary>
    /// Gets or sets the target encoding.
    /// </summary>
    public string TargetEncoding { get; init; } = string.Empty;

    /// <summary>
    /// Gets or sets the confidence level of the encoding detection (0.0 to 1.0).
    /// </summary>
    public double DetectionConfidence { get; init; }

    /// <summary>
    /// Gets or sets the backup file path if created.
    /// </summary>
    public string? BackupPath { get; init; }

    /// <summary>
    /// Gets or sets whether the file was actually converted (false if skipped because already in target encoding).
    /// </summary>
    public bool WasConverted { get; init; }
}
