// SPDX-License-Identifier: Apache-2.0

use std::collections::HashMap;

use rtnetlink::Handle;
use serde::{Deserialize, Serialize};

use crate::{Iface, IfaceType, NisporError};

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone, Default)]
#[non_exhaustive]
pub struct VethInfo {
    // Interface name of peer.
    // Use interface index number when peer interface is in other namespace.
    pub peer: String,
}

pub type VethConf = VethInfo;

impl VethConf {
    pub(crate) async fn create(
        &self,
        handle: &Handle,
        name: &str,
    ) -> Result<(), NisporError> {
        match handle
            .link()
            .add()
            .veth(name.to_string(), self.peer.clone())
            .execute()
            .await
        {
            Ok(_) => Ok(()),
            Err(e) => Err(NisporError::bug(format!(
                "Failed to create new veth pair '{}' '{}': {}",
                &name, &self.peer, e
            ))),
        }
    }
}

pub(crate) fn veth_iface_tidy_up(iface_states: &mut HashMap<String, Iface>) {
    let mut index_to_name = HashMap::new();
    for iface in iface_states.values() {
        index_to_name.insert(format!("{}", iface.index), iface.name.clone());
    }

    for iface in iface_states.values_mut() {
        if iface.iface_type != IfaceType::Veth {
            continue;
        }
        // If the link_netnsid is set, the veth peer is on a different netns
        // and therefore Nispor should use the ifindex instead.
        if iface.link_netnsid.is_some() {
            continue;
        }

        if let Some(VethInfo { peer }) = &iface.veth {
            if let Some(peer_iface_name) = index_to_name.get(peer) {
                iface.veth = Some(VethInfo {
                    peer: peer_iface_name.clone(),
                })
            }
        }
    }
}
