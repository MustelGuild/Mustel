using FurLab.Core.Services.Docker;

namespace FurLab.Core.Interfaces;

/// <summary>
/// Provides Docker container management operations.
/// </summary>
public interface IDockerService
{
    /// <summary>
    /// Checks Docker availability, distinguishing between not installed and not running.
    /// </summary>
    /// <returns>A <see cref="DockerStatus"/> value indicating the Docker state.</returns>
    DockerStatus GetDockerStatus();

    /// <summary>
    /// Checks if a container with the specified name exists.
    /// </summary>
    /// <param name="containerName">The name of the container.</param>
    /// <returns>A tuple containing existence status and current status.</returns>
    (bool exists, string? status) ContainerExists(string containerName);

    /// <summary>
    /// Starts an existing container.
    /// </summary>
    /// <param name="containerName">The name of the container to start.</param>
    /// <returns>True if successful.</returns>
    bool StartContainer(string containerName);

    /// <summary>
    /// Creates a PostgreSQL container with the configured settings.
    /// </summary>
    /// <returns>The container ID.</returns>
    string CreatePostgresContainer();
}
