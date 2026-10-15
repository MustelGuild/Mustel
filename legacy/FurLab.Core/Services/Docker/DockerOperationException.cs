namespace FurLab.Core.Services.Docker;

/// <summary>
/// Exception thrown when a Docker operation fails.
/// </summary>
public class DockerOperationException : Exception
{
    /// <summary>
    /// Initializes a new instance of the <see cref="DockerOperationException"/> class.
    /// </summary>
    /// <param name="message">The error message.</param>
    public DockerOperationException(string message) : base(message)
    {
    }
}
