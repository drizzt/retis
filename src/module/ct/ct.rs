use anyhow::{bail, Result};

use super::{bpf::CtEventFactory, ct_hook};
use crate::{
    cli::{dynamic::DynamicCommand, CliConfig},
    collect::Collector,
    core::{
        events::EventSectionFactory,
        inspect,
        probe::{Hook, ProbeManager},
    },
    module::{Module, ModuleId},
};

pub(crate) struct CtModule {
    // Whether the event capturing module was initialized
    init: bool,
}

impl Collector for CtModule {
    fn new() -> Result<Self> {
        Ok(CtModule { init: false })
    }

    fn known_kernel_types(&self) -> Option<Vec<&'static str>> {
        Some(vec!["struct sk_buff *"])
    }

    fn register_cli(&self, cmd: &mut DynamicCommand) -> Result<()> {
        cmd.register_module_noargs(ModuleId::Ct)
    }

    fn can_run(&mut self, _cli: &CliConfig) -> Result<()> {
        let kernel = &inspect::inspector()?.kernel;

        match kernel.get_config_option("CONFIG_NF_CONNTRACK") {
            Ok(Some("y")) => (),
            Ok(Some("m")) => {
                if kernel.is_module_loaded("nf_conntrack") == Some(false) {
                    bail!("'nf_conntrack' is not loaded");
                }
            }
            // If the Kernel Config is not available, the collector is not guaranteed
            // to work, but let's try.
            Err(_) => (),
            _ => bail!("This kernel does not support connection tracking"),
        }
        Ok(())
    }

    fn init(&mut self, _cli: &CliConfig, probes: &mut ProbeManager) -> Result<()> {
        // Register our generic conntrack hook.
        probes.register_kernel_hook(Hook::from(ct_hook::DATA))?;
        self.init = true;
        Ok(())
    }
}

impl Module for CtModule {
    fn collector(&mut self) -> &mut dyn Collector {
        self
    }
    fn section_factory(&self) -> Result<Box<dyn EventSectionFactory>> {
        Ok(Box::new(match self.init {
            true => CtEventFactory::bpf()?,
            false => CtEventFactory::new()?,
        }))
    }
}
