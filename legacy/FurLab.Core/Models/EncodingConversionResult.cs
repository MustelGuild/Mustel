namespace FurLab.Core.Models;

/// <summary>
/// Represents the result of an encoding conversion operation.
/// </summary>
public record EncodingConversionResult
{
    /// <summary>
    /// Gets or sets the total number of files processed.
    /// </summary>
    public int TotalFiles { get; init; }

    /// <summary>
    /// Gets or sets the number of files successfully converted.
    /// </summary>
    public int ConvertedCount { get; init; }

    /// <summary>
    /// Gets or sets the number of files skipped (already in target encoding).
    /// </summary>
    public int SkippedCount { get; init; }

    /// <summary>
    /// Gets or sets the number of files with errors.
    /// </summary>
    public int ErrorCount { get; init; }

    /// <summary>
    /// Gets or sets the list of converted files.
    /// </summary>
    public IReadOnlyList<ProcessedFileInfo> ProcessedFiles { get; init; } = [];

    /// <summary>
    /// Gets or sets the list of errors that occurred.
    /// </summary>
    public IReadOnlyList<ConversionErrorInfo> Errors { get; init; } = [];

    /// <summary>
    /// Gets whether all operations completed successfully.
    /// </summary>
    public bool Success => ErrorCount == 0;
}
