// SPDX-License-Identifier: Apache-2.0

use std::collections::HashMap;

use netlink_packet_route::{
    link::nlas, LinkMessage, ARPHRD_ETHER, ARPHRD_INFINIBAND, ARPHRD_LOOPBACK,
    IFF_ALLMULTI, IFF_AUTOMEDIA, IFF_BROADCAST, IFF_DEBUG, IFF_DORMANT,
    IFF_LOOPBACK, IFF_LOWER_UP, IFF_MASTER, IFF_MULTICAST, IFF_NOARP,
    IFF_POINTOPOINT, IFF_PORTSEL, IFF_PROMISC, IFF_RUNNING, IFF_UP,
};
use serde::{Deserialize, Serialize};

use crate::{
    ip::{fill_af_spec_inet_info, IpConf, Ipv4Info, Ipv6Info},
    mac::{mac_str_to_raw, parse_as_mac},
    mptcp::MptcpAddress,
    NisporError, VfInfo,
};

use super::{
    bond::{
        get_bond_info, get_bond_subordinate_info, BondInfo, BondSubordinateInfo,
    },
    bridge::{
        get_bridge_info, get_bridge_port_info, parse_bridge_vlan_info,
        BridgeConf, BridgeInfo, BridgePortInfo,
    },
    ethtool::EthtoolInfo,
    inter_ifaces::change_ifaces,
    ipoib::{get_ipoib_info, IpoibInfo},
    mac_vlan::{get_mac_vlan_info, MacVlanInfo},
    mac_vtap::{get_mac_vtap_info, MacVtapInfo},
    sriov::{get_sriov_info, SriovInfo},
    tun::{get_tun_info, TunInfo},
    veth::{VethConf, VethInfo},
    vlan::{get_vlan_info, VlanConf, VlanInfo},
    vrf::{
        get_vrf_info, get_vrf_subordinate_info, VrfInfo, VrfSubordinateInfo,
    },
    vxlan::{get_vxlan_info, VxlanInfo},
};

const IFF_PORT: u32 = 0x800;

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum IfaceType {
    Bond,
    Veth,
    Bridge,
    Vlan,
    Dummy,
    Vxlan,
    Loopback,
    Ethernet,
    Infiniband,
    Vrf,
    Tun,
    MacVlan,
    MacVtap,
    OpenvSwitch,
    Ipoib,
    Unknown,
    Other(String),
}

impl Default for IfaceType {
    fn default() -> Self {
        IfaceType::Unknown
    }
}

impl std::fmt::Display for IfaceType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::Bond => "bond",
                Self::Veth => "veth",
                Self::Bridge => "bridge",
                Self::Vlan => "vlan",
                Self::Dummy => "dummy",
                Self::Vxlan => "vxlan",
                Self::Loopback => "loopback",
                Self::Ethernet => "ethernet",
                Self::Infiniband => "infiniband",
                Self::Vrf => "vrf",
                Self::Tun => "tun",
                Self::MacVlan => "macvlan",
                Self::MacVtap => "macvtap",
                Self::OpenvSwitch => "openvswitch",
                Self::Ipoib => "ipoib",
                Self::Unknown => "unknown",
                Self::Other(s) => s,
            }
        )
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum IfaceState {
    Up,
    Dormant,
    Down,
    LowerLayerDown,
    Absent, // Only for IfaceConf
    Other(String),
    Unknown,
}

impl Default for IfaceState {
    fn default() -> Self {
        IfaceState::Unknown
    }
}

impl std::fmt::Display for IfaceState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::Up => "up",
                Self::Dormant => "dormant",
                Self::Down => "down",
                Self::LowerLayerDown => "lower_layer_down",
                Self::Absent => "absent",
                Self::Other(s) => s.as_str(),
                Self::Unknown => "unknown",
            }
        )
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum IfaceFlags {
    AllMulti,
    AutoMedia,
    Broadcast,
    Debug,
    Dormant,
    Loopback,
    LowerUp,
    Controller,
    Multicast,
    NoArp,
    PoinToPoint,
    Portsel,
    Promisc,
    Running,
    Subordinate,
    Up,
    Other(u32),
    Unknown,
}

impl Default for IfaceFlags {
    fn default() -> Self {
        Self::Unknown
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ControllerType {
    Bond,
    Bridge,
    Vrf,
    OpenvSwitch,
    Other(String),
    Unknown,
}

impl From<&str> for ControllerType {
    fn from(s: &str) -> Self {
        match s {
            "bond" => ControllerType::Bond,
            "bridge" => ControllerType::Bridge,
            "vrf" => ControllerType::Vrf,
            "openvswitch" => ControllerType::OpenvSwitch,
            _ => ControllerType::Other(s.to_string()),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone, Default)]
#[non_exhaustive]
pub struct Iface {
    pub name: String,
    #[serde(skip_serializing)]
    pub index: u32,
    pub iface_type: IfaceType,
    pub state: IfaceState,
    pub mtu: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_mtu: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_mtu: Option<i64>,
    pub flags: Vec<IfaceFlags>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ipv4: Option<Ipv4Info>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ipv6: Option<Ipv6Info>,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub mac_address: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub permanent_mac_address: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub controller: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub controller_type: Option<ControllerType>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub link_netnsid: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ethtool: Option<EthtoolInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bond: Option<BondInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bond_subordinate: Option<BondSubordinateInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bridge: Option<BridgeInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bridge_port: Option<BridgePortInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tun: Option<TunInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vlan: Option<VlanInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vxlan: Option<VxlanInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub veth: Option<VethInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vrf: Option<VrfInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vrf_subordinate: Option<VrfSubordinateInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mac_vlan: Option<MacVlanInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mac_vtap: Option<MacVtapInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sriov: Option<SriovInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sriov_vf: Option<VfInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ipoib: Option<IpoibInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mptcp: Option<Vec<MptcpAddress>>,
}

// TODO: impl From Iface to IfaceConf

pub(crate) fn parse_nl_msg_to_name_and_index(
    nl_msg: &LinkMessage,
) -> Option<(String, u32)> {
    let index = nl_msg.header.index;
    let name = _get_iface_name(nl_msg);
    if name.is_empty() {
        None
    } else {
        Some((name, index))
    }
}

pub(crate) fn parse_nl_msg_to_iface(
    nl_msg: &LinkMessage,
) -> Result<Option<Iface>, NisporError> {
    let name = _get_iface_name(nl_msg);
    if name.is_empty() {
        return Ok(None);
    }
    let link_layer_type = match nl_msg.header.link_layer_type {
        ARPHRD_ETHER => IfaceType::Ethernet,
        ARPHRD_LOOPBACK => IfaceType::Loopback,
        ARPHRD_INFINIBAND => IfaceType::Infiniband,
        _ => IfaceType::Unknown,
    };
    let mut iface_state = Iface {
        name,
        iface_type: link_layer_type.clone(),
        ..Default::default()
    };
    iface_state.index = nl_msg.header.index;
    let mut link: Option<u32> = None;
    for nla in &nl_msg.nlas {
        if let nlas::Nla::Mtu(mtu) = nla {
            iface_state.mtu = *mtu as i64;
        } else if let nlas::Nla::MinMtu(mtu) = nla {
            iface_state.min_mtu =
                if *mtu != 0 { Some(*mtu as i64) } else { None };
        } else if let nlas::Nla::MaxMtu(mtu) = nla {
            iface_state.max_mtu =
                if *mtu != 0 { Some(*mtu as i64) } else { None };
        } else if let nlas::Nla::Address(mac) = nla {
            iface_state.mac_address = parse_as_mac(mac.len(), mac)?;
        } else if let nlas::Nla::PermAddress(mac) = nla {
            iface_state.permanent_mac_address = parse_as_mac(mac.len(), mac)?;
        } else if let nlas::Nla::OperState(state) = nla {
            iface_state.state = _get_iface_state(state);
        } else if let nlas::Nla::Master(controller) = nla {
            iface_state.controller = Some(format!("{controller}"));
        } else if let nlas::Nla::Link(l) = nla {
            link = Some(*l);
        } else if let nlas::Nla::Info(infos) = nla {
            for info in infos {
                if let nlas::Info::Kind(t) = info {
                    let iface_type = match t {
                        nlas::InfoKind::Bond => IfaceType::Bond,
                        nlas::InfoKind::Veth => IfaceType::Veth,
                        nlas::InfoKind::Bridge => IfaceType::Bridge,
                        nlas::InfoKind::Vlan => IfaceType::Vlan,
                        nlas::InfoKind::Vxlan => IfaceType::Vxlan,
                        nlas::InfoKind::Dummy => IfaceType::Dummy,
                        nlas::InfoKind::Tun => IfaceType::Tun,
                        nlas::InfoKind::Vrf => IfaceType::Vrf,
                        nlas::InfoKind::MacVlan => IfaceType::MacVlan,
                        nlas::InfoKind::MacVtap => IfaceType::MacVtap,
                        nlas::InfoKind::Ipoib => IfaceType::Ipoib,
                        nlas::InfoKind::Other(s) => match s.as_ref() {
                            "openvswitch" => IfaceType::OpenvSwitch,
                            _ => IfaceType::Other(s.clone()),
                        },
                        _ => IfaceType::Other(format!("{t:?}")),
                    };
                    if let IfaceType::Other(_) = iface_type {
                        /* We did not find an explicit link type. Instead it's
                         * just "Other(_)". If we already determined a link type
                         * above (ethernet or infiniband), keep that one. */
                        if iface_state.iface_type == IfaceType::Unknown {
                            iface_state.iface_type = iface_type
                        }
                    } else {
                        /* We found a better link type based on the kind. Use it. */
                        iface_state.iface_type = iface_type
                    }
                }
            }
            for info in infos {
                if let nlas::Info::Data(d) = info {
                    match iface_state.iface_type {
                        IfaceType::Bond => iface_state.bond = get_bond_info(d)?,
                        IfaceType::Bridge => {
                            iface_state.bridge = get_bridge_info(d)?
                        }
                        IfaceType::Tun => match get_tun_info(d) {
                            Ok(info) => {
                                iface_state.tun = Some(info);
                            }
                            Err(e) => {
                                log::warn!("Error parsing TUN info: {}", e);
                            }
                        },
                        IfaceType::Vlan => iface_state.vlan = get_vlan_info(d),
                        IfaceType::Vxlan => {
                            iface_state.vxlan = get_vxlan_info(d)?
                        }
                        IfaceType::Vrf => iface_state.vrf = get_vrf_info(d),
                        IfaceType::MacVlan => {
                            iface_state.mac_vlan = get_mac_vlan_info(d)?
                        }
                        IfaceType::MacVtap => {
                            iface_state.mac_vtap = get_mac_vtap_info(d)?
                        }
                        IfaceType::Ipoib => {
                            iface_state.ipoib = get_ipoib_info(d);
                        }
                        _ => log::warn!(
                            "Unhandled IFLA_INFO_DATA for iface type {:?}",
                            iface_state.iface_type
                        ),
                    }
                }
            }
            for info in infos {
                if let nlas::Info::PortKind(d) = info {
                    match d {
                        nlas::InfoPortKind::Bond => {
                            iface_state.controller_type =
                                Some(ControllerType::Bond)
                        }
                        nlas::InfoPortKind::Other(s) => {
                            iface_state.controller_type =
                                Some(s.as_str().into())
                        }
                        _ => {
                            log::info!("Unknown port kind {:?}", info);
                        }
                    }
                }
            }
            if let Some(controller_type) = &iface_state.controller_type {
                for info in infos {
                    if let nlas::Info::PortData(d) = info {
                        match d {
                            nlas::InfoPortData::BondPort(bond_ports) => {
                                iface_state.bond_subordinate = Some(
                                    get_bond_subordinate_info(bond_ports)?,
                                );
                            }
                            nlas::InfoPortData::Other(data) => {
                                match controller_type {
                                    ControllerType::Bridge => {
                                        iface_state.bridge_port =
                                            get_bridge_port_info(data)?;
                                    }
                                    ControllerType::Vrf => {
                                        iface_state.vrf_subordinate =
                                            get_vrf_subordinate_info(data)?;
                                    }
                                    _ => log::warn!(
                                        "Unknown controller type {:?}",
                                        controller_type
                                    ),
                                }
                            }
                            _ => {
                                log::warn!("Unknown InfoPortData {:?}", d);
                            }
                        }
                    }
                }
            }
        } else if let nlas::Nla::VfInfoList(data) = nla {
            if let Ok(info) =
                get_sriov_info(&iface_state.name, data, &link_layer_type)
            {
                iface_state.sriov = Some(info);
            }
        } else if let nlas::Nla::NetnsId(id) = nla {
            iface_state.link_netnsid = Some(*id);
        } else if let nlas::Nla::AfSpecInet(inet_nla) = nla {
            fill_af_spec_inet_info(&mut iface_state, inet_nla.as_slice());
        } else {
            // Place holder for paring more Nla
        }
    }
    if let Some(ref mut vlan_info) = iface_state.vlan {
        if let Some(base_iface_index) = link {
            vlan_info.base_iface = format!("{base_iface_index}");
        }
    }
    if let Some(ref mut ib_info) = iface_state.ipoib {
        if let Some(base_iface_index) = link {
            ib_info.base_iface = Some(format!("{base_iface_index}"));
        }
    }
    if let Some(iface_index) = link {
        match iface_state.iface_type {
            IfaceType::Veth => {
                iface_state.veth = Some(VethInfo {
                    peer: format!("{iface_index}"),
                })
            }
            IfaceType::MacVlan => {
                if let Some(ref mut mac_vlan_info) = iface_state.mac_vlan {
                    mac_vlan_info.base_iface = format!("{iface_index}");
                }
            }
            IfaceType::MacVtap => {
                if let Some(ref mut mac_vtap_info) = iface_state.mac_vtap {
                    mac_vtap_info.base_iface = format!("{iface_index}");
                }
            }
            _ => (),
        }
    }
    iface_state.flags = _parse_iface_flags(nl_msg.header.flags);
    Ok(Some(iface_state))
}

fn _get_iface_name(nl_msg: &LinkMessage) -> String {
    for nla in &nl_msg.nlas {
        if let nlas::Nla::IfName(name) = nla {
            return name.clone();
        }
    }
    "".into()
}

pub(crate) fn fill_bridge_vlan_info(
    iface_states: &mut HashMap<String, Iface>,
    nl_msg: &LinkMessage,
) -> Result<(), NisporError> {
    let name = _get_iface_name(nl_msg);
    if name.is_empty() {
        return Ok(());
    }
    if let Some(iface_state) = iface_states.get_mut(&name) {
        for nla in &nl_msg.nlas {
            if let nlas::Nla::AfSpecBridge(nlas) = nla {
                parse_bridge_vlan_info(iface_state, nlas)?;
            }
        }
    }
    Ok(())
}

fn _get_iface_state(state: &nlas::State) -> IfaceState {
    match state {
        nlas::State::Up => IfaceState::Up,
        nlas::State::Dormant => IfaceState::Dormant,
        nlas::State::Down => IfaceState::Down,
        nlas::State::LowerLayerDown => IfaceState::LowerLayerDown,
        nlas::State::Unknown => IfaceState::Unknown,
        _ => IfaceState::Other(format!("{state:?}")),
    }
}

fn _parse_iface_flags(flags: u32) -> Vec<IfaceFlags> {
    let mut ret = Vec::new();
    if (flags & IFF_ALLMULTI) > 0 {
        ret.push(IfaceFlags::AllMulti)
    }
    if (flags & IFF_AUTOMEDIA) > 0 {
        ret.push(IfaceFlags::AutoMedia)
    }
    if (flags & IFF_BROADCAST) > 0 {
        ret.push(IfaceFlags::Broadcast)
    }
    if (flags & IFF_DEBUG) > 0 {
        ret.push(IfaceFlags::Debug)
    }
    if (flags & IFF_DORMANT) > 0 {
        ret.push(IfaceFlags::Dormant)
    }
    if (flags & IFF_LOOPBACK) > 0 {
        ret.push(IfaceFlags::Loopback)
    }
    if (flags & IFF_LOWER_UP) > 0 {
        ret.push(IfaceFlags::LowerUp)
    }
    if (flags & IFF_MASTER) > 0 {
        ret.push(IfaceFlags::Controller)
    }
    if (flags & IFF_MULTICAST) > 0 {
        ret.push(IfaceFlags::Multicast)
    }
    if (flags & IFF_NOARP) > 0 {
        ret.push(IfaceFlags::NoArp)
    }
    if (flags & IFF_POINTOPOINT) > 0 {
        ret.push(IfaceFlags::PoinToPoint)
    }
    if (flags & IFF_PORTSEL) > 0 {
        ret.push(IfaceFlags::Portsel)
    }
    if (flags & IFF_PROMISC) > 0 {
        ret.push(IfaceFlags::Promisc)
    }
    if (flags & IFF_RUNNING) > 0 {
        ret.push(IfaceFlags::Running)
    }
    if (flags & IFF_PORT) > 0 {
        ret.push(IfaceFlags::Subordinate)
    }
    if (flags & IFF_UP) > 0 {
        ret.push(IfaceFlags::Up)
    }

    ret
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone, Default)]
#[non_exhaustive]
pub struct IfaceConf {
    pub name: String,
    #[serde(default = "default_iface_state_in_conf")]
    pub state: IfaceState,
    #[serde(rename = "type")]
    pub iface_type: Option<IfaceType>,
    pub controller: Option<String>,
    pub ipv4: Option<IpConf>,
    pub ipv6: Option<IpConf>,
    pub mac_address: Option<String>,
    pub veth: Option<VethConf>,
    pub bridge: Option<BridgeConf>,
    pub vlan: Option<VlanConf>,
}

impl IfaceConf {
    pub async fn apply(&self, cur_iface: &Iface) -> Result<(), NisporError> {
        log::warn!(
            "WARN: IfaceConf::apply() is deprecated, \
            please use NetConf::apply() instead"
        );
        let ifaces = vec![self];
        let mut cur_ifaces = HashMap::new();
        cur_ifaces.insert(self.name.to_string(), cur_iface.clone());
        change_ifaces(&ifaces, &cur_ifaces).await
    }
}

fn default_iface_state_in_conf() -> IfaceState {
    IfaceState::Up
}

pub(crate) async fn change_iface_state(
    handle: &rtnetlink::Handle,
    index: u32,
    up: bool,
) -> Result<(), NisporError> {
    if up {
        handle.link().set(index).up().execute().await?;
    } else {
        handle.link().set(index).down().execute().await?;
    }
    Ok(())
}

pub(crate) async fn change_iface_mac(
    handle: &rtnetlink::Handle,
    index: u32,
    mac_address: &str,
) -> Result<(), NisporError> {
    change_iface_state(handle, index, false).await?;
    handle
        .link()
        .set(index)
        .address(mac_str_to_raw(mac_address)?)
        .execute()
        .await?;
    Ok(())
}
