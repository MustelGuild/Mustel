namespace FurLab.Core.Models;

/// <summary>
/// Result of a PgPassService operation.
/// </summary>
public record PgPassResult
{
    /// <summary>Indicates whether the operation completed without error (including duplicates, which are not errors).</summary>
    public bool Success { get; init; }

    /// <summary>Descriptive message of the result for user display.</summary>
    public string Message { get; init; } = string.Empty;

    /// <summary>
    /// Indicates that the entry already existed in the file (desired state already present).
    /// Success = true when IsDuplicate = true — duplicate is not an error; exit code remains 0.
    /// </summary>
    public bool IsDuplicate { get; init; }

    /// <summary>Creates a success result.</summary>
    public static PgPassResult Ok(string message)
        => new() { Success = true, Message = message };

    /// <summary>Creates an informative duplicate result (success, without file modification).</summary>
    public static PgPassResult Duplicate(string message)
        => new() { Success = true, IsDuplicate = true, Message = message };

    /// <summary>Creates a failure result with an actionable message.</summary>
    public static PgPassResult Fail(string message)
        => new() { Success = false, Message = message };
}
