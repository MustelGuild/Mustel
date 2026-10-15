using FurLab.Core.Interfaces;
using FurLab.Core.Services.Docker;
using Spectre.Console.Cli;

namespace FurLab.CLI.Commands.Docker.Postgres;

/// <summary>
/// Creates or starts a PostgreSQL Docker container for local development.
/// </summary>
public sealed class DockerPostgresCommand : AsyncCommand<DockerPostgresSettings>
{
    private readonly IDockerService _dockerService;

    /// <summary>
    /// Initializes a new instance of the <see cref="DockerPostgresCommand"/> class.
    /// </summary>
    /// <param name="dockerService">The Docker service.</param>
    public DockerPostgresCommand(IDockerService dockerService)
    {
        _dockerService = dockerService;
    }

    /// <inheritdoc/>
    protected override Task<int> ExecuteAsync(CommandContext context, DockerPostgresSettings settings, CancellationToken cancellation)
    {
        var dockerStatus = _dockerService.GetDockerStatus();

        if (dockerStatus == DockerStatus.NotInstalled)
        {
            Console.WriteLine("Error: Docker not found. Install Docker Desktop: https://www.docker.com/products/docker-desktop");
            return Task.FromResult(1);
        }

        if (dockerStatus == DockerStatus.NotRunning)
        {
            Console.WriteLine("Error: Docker is installed but not running. Start Docker Desktop and try again.");
            return Task.FromResult(1);
        }

        var containerName = DockerConstants.PostgresContainerName;
        var (exists, status) = _dockerService.ContainerExists(containerName);

        if (exists)
        {
            Console.WriteLine($"Container '{containerName}' already exists (Status: {status ?? ""}). Starting...");
            _dockerService.StartContainer(containerName);
            DisplayConnectionInfo();
        }
        else
        {
            Console.WriteLine("Creating PostgreSQL container...");
            var containerId = _dockerService.CreatePostgresContainer();
            Console.WriteLine("Container created successfully!");
            Console.WriteLine($"Container ID: {containerId ?? "unknown"}");
            DisplayConnectionInfo();
        }

        return Task.FromResult(0);
    }

    private static void DisplayConnectionInfo()
    {
        Console.WriteLine();
        Console.WriteLine("========================================");
        Console.WriteLine("PostgreSQL is ready!");
        Console.WriteLine("========================================");
        Console.WriteLine();
        Console.WriteLine("Connection string:");
        Console.WriteLine($"  Host=localhost;Port={DockerConstants.PostgresPort};Database={DockerConstants.PostgresDatabase};Username={DockerConstants.PostgresUsername};Password={DockerConstants.PostgresPassword}");
        Console.WriteLine();
        Console.WriteLine("Or via psql:");
        Console.WriteLine($"  psql -h localhost -p {DockerConstants.PostgresPort} -U {DockerConstants.PostgresUsername} -d {DockerConstants.PostgresDatabase}");
        Console.WriteLine();
    }
}
