using FurLab.Core.Interfaces;
using FurLab.Core.Services;

using Microsoft.Extensions.DependencyInjection;
using Microsoft.Extensions.Logging;

using Microsoft.VisualStudio.TestTools.UnitTesting;

namespace FurLab.Tests.Commands;

[TestClass]
public class DatabaseCommandTests
{
    [TestMethod]
    public void DatabaseService_CanBeResolvedFromDI()
    {
        var services = new ServiceCollection();
        services.AddFurLabServices();
        services.AddLogging(builder => builder.AddDebug());

        var serviceProvider = services.BuildServiceProvider();

        var databaseService = serviceProvider.GetService<IDatabaseService>();
        Assert.IsNotNull(databaseService);
    }

    [TestMethod]
    public void IPostgresBinaryLocator_CanBeResolvedFromDI()
    {
        var services = new ServiceCollection();
        services.AddFurLabServices();
        services.AddLogging(builder => builder.AddDebug());

        var serviceProvider = services.BuildServiceProvider();

        var locator = serviceProvider.GetService<IPostgresBinaryLocator>();
        Assert.IsNotNull(locator);
    }
}
