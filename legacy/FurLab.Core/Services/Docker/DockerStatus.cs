namespace FurLab.Core.Services.Docker;

/// <summary>
/// Represents the Docker daemon availability state.
/// </summary>
public enum DockerStatus
{
    /// <summary>Docker is installed and the daemon is running.</summary>
    Running,

    /// <summary>Docker is installed but the daemon is not running.</summary>
    NotRunning,

    /// <summary>Docker executable was not found on the system PATH.</summary>
    NotInstalled
}
