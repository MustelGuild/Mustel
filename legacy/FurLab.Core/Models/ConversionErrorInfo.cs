namespace FurLab.Core.Models;

/// <summary>
/// Information about a conversion error.
/// </summary>
public record ConversionErrorInfo
{
    /// <summary>
    /// Gets or sets the file path that caused the error.
    /// </summary>
    public string FilePath { get; init; } = string.Empty;

    /// <summary>
    /// Gets or sets the error message.
    /// </summary>
    public string ErrorMessage { get; init; } = string.Empty;

    /// <summary>
    /// Gets or sets the type of error.
    /// </summary>
    public ConversionErrorType ErrorType { get; init; }
}
