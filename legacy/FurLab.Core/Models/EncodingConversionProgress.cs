namespace FurLab.Core.Models;

/// <summary>
/// Represents progress information for an encoding conversion operation.
/// </summary>
public record EncodingConversionProgress
{
    /// <summary>
    /// Gets or sets the total number of files to process.
    /// </summary>
    public int TotalFiles { get; init; }

    /// <summary>
    /// Gets or sets the number of files processed so far.
    /// </summary>
    public int ProcessedFiles { get; init; }

    /// <summary>
    /// Gets or sets the current file being processed.
    /// </summary>
    public string CurrentFile { get; init; } = string.Empty;

    /// <summary>
    /// Gets or sets the current operation message.
    /// </summary>
    public string Message { get; init; } = string.Empty;

    /// <summary>
    /// Gets or sets the percentage complete (0.0 to 100.0).
    /// </summary>
    public double PercentComplete => TotalFiles > 0 ? (ProcessedFiles / (double)TotalFiles) * 100 : 0;
}
