namespace FurLab.Core.Models;

/// <summary>
/// Types of conversion errors.
/// </summary>
public enum ConversionErrorType
{
    /// <summary>
    /// Unknown error.
    /// </summary>
    Unknown,

    /// <summary>
    /// Encoding detection failed or confidence too low.
    /// </summary>
    DetectionFailed,

    /// <summary>
    /// File not found.
    /// </summary>
    FileNotFound,

    /// <summary>
    /// Permission denied when reading or writing file.
    /// </summary>
    PermissionDenied,

    /// <summary>
    /// Path traversal detected.
    /// </summary>
    InvalidPath,

    /// <summary>
    /// Error reading file.
    /// </summary>
    ReadError,

    /// <summary>
    /// Error writing file.
    /// </summary>
    WriteError,
}
