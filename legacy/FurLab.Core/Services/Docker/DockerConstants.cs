namespace FurLab.Core.Services.Docker;

/// <summary>
/// Contains constant values for Docker operations.
/// </summary>
public static class DockerConstants
{
    /// <summary>
    /// The Docker executable name.
    /// </summary>
    public const string DockerExecutable = "docker";

    /// <summary>
    /// The PostgreSQL container name.
    /// </summary>
    public const string PostgresContainerName = "postgres-ptbr";

    /// <summary>
    /// The PostgreSQL Docker image.
    /// </summary>
    public const string PostgresImage = "postgres:alpine";

    /// <summary>
    /// The PostgreSQL volume name for data persistence.
    /// </summary>
    public const string PostgresVolumeName = "postgres-data";

    /// <summary>
    /// The default PostgreSQL password.
    /// </summary>
    public const string PostgresPassword = "dev";

    /// <summary>
    /// The PostgreSQL port.
    /// </summary>
    public const string PostgresPort = "5432";

    /// <summary>
    /// The default PostgreSQL database name.
    /// </summary>
    public const string PostgresDatabase = "postgres";

    /// <summary>
    /// The default PostgreSQL username.
    /// </summary>
    public const string PostgresUsername = "postgres";
}
