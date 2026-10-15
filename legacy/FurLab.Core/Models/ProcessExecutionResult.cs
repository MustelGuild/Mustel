namespace FurLab.Core.Models;

/// <summary>
/// Represents the result of a process execution.
/// </summary>
public record ProcessExecutionResult
{
    /// <summary>
    /// Gets or sets the exit code of the process.
    /// </summary>
    public int ExitCode { get; init; }

    /// <summary>
    /// Gets or sets the standard output of the process.
    /// </summary>
    public string StandardOutput { get; init; } = string.Empty;

    /// <summary>
    /// Gets or sets the standard error of the process.
    /// </summary>
    public string StandardError { get; init; } = string.Empty;

    /// <summary>
    /// Gets or sets whether the process completed successfully.
    /// </summary>
    public bool Success => ExitCode == 0;
}
