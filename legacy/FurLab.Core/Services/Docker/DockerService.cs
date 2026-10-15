using System.Diagnostics;

using FurLab.Core.Interfaces;

namespace FurLab.Core.Services.Docker;

/// <summary>
/// Service for Docker container operations.
/// </summary>
public class DockerService : IDockerService
{
    /// <inheritdoc/>
    public DockerStatus GetDockerStatus()
    {
        try
        {
            var startInfo = new ProcessStartInfo
            {
                FileName = DockerConstants.DockerExecutable,
                RedirectStandardOutput = true,
                RedirectStandardError = true,
                UseShellExecute = false,
                CreateNoWindow = true
            };
            startInfo.ArgumentList.Add("info");

            using var process = Process.Start(startInfo);
            if (process == null)
            {
                return DockerStatus.NotInstalled;
            }

            process.WaitForExit();
            return process.ExitCode == 0 ? DockerStatus.Running : DockerStatus.NotRunning;
        }
        catch (System.ComponentModel.Win32Exception)
        {
            return DockerStatus.NotInstalled;
        }
        catch
        {
            return DockerStatus.NotInstalled;
        }
    }

    /// <inheritdoc/>
    public (bool exists, string? status) ContainerExists(string containerName)
    {
        try
        {
            var startInfo = new ProcessStartInfo
            {
                FileName = DockerConstants.DockerExecutable,
                RedirectStandardOutput = true,
                RedirectStandardError = true,
                UseShellExecute = false,
                CreateNoWindow = true
            };
            startInfo.ArgumentList.Add("ps");
            startInfo.ArgumentList.Add("-a");
            startInfo.ArgumentList.Add("--filter");
            startInfo.ArgumentList.Add($"name=^{containerName}$");
            startInfo.ArgumentList.Add("--format");
            startInfo.ArgumentList.Add("{{.Status}}");

            using var process = Process.Start(startInfo);
            if (process == null)
            {
                return (false, null);
            }

            var output = process.StandardOutput.ReadToEnd().Trim();
            process.WaitForExit();

            if (!string.IsNullOrEmpty(output))
            {
                return (true, output);
            }

            return (false, null);
        }
        catch
        {
            return (false, null);
        }
    }

    /// <inheritdoc/>
    public bool StartContainer(string containerName)
    {
        var startInfo = new ProcessStartInfo
        {
            FileName = DockerConstants.DockerExecutable,
            RedirectStandardOutput = true,
            RedirectStandardError = true,
            UseShellExecute = false,
            CreateNoWindow = true
        };
        startInfo.ArgumentList.Add("start");
        startInfo.ArgumentList.Add(containerName);

        using var process = Process.Start(startInfo);
        if (process == null)
        {
            throw new DockerOperationException("Failed to start docker process.");
        }

        var output = process.StandardOutput.ReadToEnd();
        var error = process.StandardError.ReadToEnd();
        process.WaitForExit();

        if (process.ExitCode != 0)
        {
            throw new DockerOperationException($"Failed to start container: {error}");
        }

        return true;
    }

    /// <inheritdoc/>
    public string CreatePostgresContainer()
    {
        var startInfo = new ProcessStartInfo
        {
            FileName = DockerConstants.DockerExecutable,
            RedirectStandardOutput = true,
            RedirectStandardError = true,
            UseShellExecute = false,
            CreateNoWindow = true
        };
        foreach (var arg in BuildPostgresRunArguments())
        {
            startInfo.ArgumentList.Add(arg);
        }

        using var process = Process.Start(startInfo) ?? throw new DockerOperationException("Failed to start docker process.");

        var output = process.StandardOutput.ReadToEnd();
        var error = process.StandardError.ReadToEnd();
        process.WaitForExit();

        if (process.ExitCode != 0)
        {
            throw new DockerOperationException($"Failed to create container: {error}");
        }

        return output.Trim();
    }

    private static List<string> BuildPostgresRunArguments()
    {
        return
        [
            "run",
            "-d",
            "--name", DockerConstants.PostgresContainerName,
            "--restart", "always",
            "-e", "POSTGRES_PASSWORD=dev",
            "-e", "LANG=pt_BR.UTF-8",
            "-e", "LC_ALL=pt_BR.UTF-8",
            "-p", $"{DockerConstants.PostgresPort}:{DockerConstants.PostgresPort}",
            "-v", $"{DockerConstants.PostgresVolumeName}:/var/lib/postgresql/data",
            DockerConstants.PostgresImage,
            "postgres",
            "-c", "log_statement=all",
            "-c", "log_min_duration_statement=0"
        ];
    }
}
