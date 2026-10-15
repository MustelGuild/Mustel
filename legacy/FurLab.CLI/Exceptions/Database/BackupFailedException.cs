namespace FurLab.CLI.Exceptions.Database;

/// <summary>
/// Exception thrown when a database backup fails.
/// </summary>
public class BackupFailedException : Exception
{
    /// <summary>
    /// Initializes a new instance of the <see cref="BackupFailedException"/> class.
    /// </summary>
    /// <param name="message">The error message.</param>
    public BackupFailedException(string message) : base(message)
    {
    }

    /// <summary>
    /// Initializes a new instance of the <see cref="BackupFailedException"/> class.
    /// </summary>
    /// <param name="message">The error message.</param>
    /// <param name="innerException">The inner exception.</param>
    public BackupFailedException(string message, Exception innerException) : base(message, innerException)
    {
    }
}
