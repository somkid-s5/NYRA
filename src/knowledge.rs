#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KnowledgeCard {
    pub id: &'static str,
    pub title: &'static str,
    pub operating_system: &'static str,
    pub intent: &'static str,
    pub command: &'static str,
    pub parameters: &'static str,
    pub example: &'static str,
    pub expected_output: &'static str,
    pub explanation: &'static str,
    pub risk: &'static str,
    pub side_effects: &'static str,
    pub required_privileges: &'static str,
    pub related_diagnostics: &'static [&'static str],
    pub keywords: &'static [&'static str],
}

pub const KNOWLEDGE_CARDS: &[KnowledgeCard] = &[
    KnowledgeCard {
        id: "linux.filesystem-space",
        title: "Check filesystem space",
        operating_system: "Linux",
        intent: "Find mounted filesystems with low available space.",
        command: "df -h",
        parameters: "Optional mount path, such as /var.",
        example: "df -h /var",
        expected_output: "A table with filesystem, size, used, available, use percentage, and mount point.",
        explanation: "Review the Use% and Available columns to find a volume that is close to full.",
        risk: "R0 — local read-only inspection",
        side_effects: "Does not modify files or filesystem state.",
        required_privileges: "None for mounted filesystems visible to the current user.",
        related_diagnostics: &["system.disk"],
        keywords: &[
            "disk full",
            "free disk space",
            "filesystem",
            "volume",
            "df -h",
        ],
    },
    KnowledgeCard {
        id: "windows.filesystem-space",
        title: "Check Windows volume space",
        operating_system: "Windows PowerShell",
        intent: "Review total and remaining capacity for mounted volumes.",
        command: "Get-Volume | Select-Object DriveLetter, FileSystemLabel, Size, SizeRemaining",
        parameters: "No required parameters.",
        example: "Get-Volume | Select-Object DriveLetter, FileSystemLabel, Size, SizeRemaining",
        expected_output: "One row per accessible volume, with size values in bytes.",
        explanation: "Compare SizeRemaining with Size. Volumes without drive letters may still be relevant to the system.",
        risk: "R0 — local read-only inspection",
        side_effects: "Does not modify files or volume state.",
        required_privileges: "None for volumes visible to the current session.",
        related_diagnostics: &["system.disk"],
        keywords: &[
            "disk full",
            "free disk space",
            "storage",
            "volume",
            "Get-Volume",
        ],
    },
    KnowledgeCard {
        id: "linux.find-large-files",
        title: "Find large files under a directory",
        operating_system: "Linux",
        intent: "Locate files larger than a chosen size without deleting them.",
        command: "find /var/log -type f -size +100M -print",
        parameters: "Replace /var/log with a directory you are authorized to inspect; change 100M to the size threshold.",
        example: "find /var/log -type f -size +100M -print",
        expected_output: "Paths of matching regular files; no output means no matches were found in the searched tree.",
        explanation: "This command only lists matching files. Keep the search root narrow because recursive traversal can take time on large trees.",
        risk: "R1 — read-only directory traversal",
        side_effects: "Reads directory entries and file metadata; does not change or delete files.",
        required_privileges: "Read and traverse permission for the chosen directory.",
        related_diagnostics: &["system.disk", "file.sha256"],
        keywords: &["disk full", "large files", "log size", "find files", "100M"],
    },
    KnowledgeCard {
        id: "windows.find-large-files",
        title: "Find large files under a directory",
        operating_system: "Windows PowerShell",
        intent: "List the 20 largest files in a chosen directory tree.",
        command: "Get-ChildItem -LiteralPath C:\\Logs -File -Recurse -ErrorAction SilentlyContinue | Sort-Object Length -Descending | Select-Object -First 20 FullName, Length",
        parameters: "Replace C:\\Logs with an authorized directory. Recursive scans may take time.",
        example: "Get-ChildItem -LiteralPath C:\\Logs -File -Recurse -ErrorAction SilentlyContinue | Sort-Object Length -Descending | Select-Object -First 20 FullName, Length",
        expected_output: "Up to 20 paths and file lengths, sorted largest first.",
        explanation: "This enumerates files and reads metadata; it does not delete or modify them. Avoid broad roots such as C:\\ on a busy host.",
        risk: "R1 — read-only directory traversal",
        side_effects: "Reads directory entries and file metadata; does not change or delete files.",
        required_privileges: "Read and traverse permission for the chosen directory.",
        related_diagnostics: &["system.disk", "file.sha256"],
        keywords: &[
            "disk full",
            "large files",
            "storage",
            "Get-ChildItem",
            "logs",
        ],
    },
    KnowledgeCard {
        id: "linux.dns-lookup",
        title: "Look up host addresses",
        operating_system: "Linux",
        intent: "Ask the system name service for IPv4 and IPv6 addresses.",
        command: "getent ahosts example.com",
        parameters: "Replace example.com with the hostname to inspect.",
        example: "getent ahosts example.com",
        expected_output: "Resolved address rows, or no rows when the configured name service returned no result.",
        explanation: "getent uses the host's configured name service, which may include DNS, local hosts files, or other NSS sources.",
        risk: "R1 — sends a name-service query when needed",
        side_effects: "May query configured name servers for the supplied hostname; does not change local settings.",
        required_privileges: "None.",
        related_diagnostics: &["network.dns", "network.reverse_dns"],
        keywords: &[
            "dns",
            "resolve hostname",
            "address lookup",
            "getent",
            "A record",
            "AAAA record",
        ],
    },
    KnowledgeCard {
        id: "windows.dns-lookup",
        title: "Look up host addresses",
        operating_system: "Windows PowerShell",
        intent: "Query the configured resolver for IPv4 and IPv6 records.",
        command: "Resolve-DnsName -Name example.com -Type A,AAAA",
        parameters: "Replace example.com with the hostname to inspect.",
        example: "Resolve-DnsName -Name example.com -Type A,AAAA",
        expected_output: "A and AAAA records returned by the configured name server.",
        explanation: "The cmdlet uses Windows DNS client configuration. A query failure can reflect resolver reachability or policy as well as a missing record.",
        risk: "R1 — sends a DNS query",
        side_effects: "Queries the configured resolver for the supplied hostname; does not change local settings.",
        required_privileges: "None.",
        related_diagnostics: &["network.dns", "network.reverse_dns"],
        keywords: &[
            "dns",
            "resolve hostname",
            "address lookup",
            "Resolve-DnsName",
            "A record",
            "AAAA record",
        ],
    },
    KnowledgeCard {
        id: "linux.tcp-connectivity",
        title: "Check one TCP endpoint",
        operating_system: "Linux",
        intent: "Attempt a bounded TCP connection to one authorized host and port.",
        command: "nc -vz -w 5 example.com 443",
        parameters: "Replace the hostname and port; -w 5 bounds the wait to five seconds. Requires netcat (nc).",
        example: "nc -vz -w 5 example.com 443",
        expected_output: "A success or failure message from nc; exact wording depends on the installed implementation.",
        explanation: "A successful TCP handshake proves only that the endpoint accepted a connection at that moment, not that an application is healthy.",
        risk: "R1 — network connection attempt",
        side_effects: "Sends a connection attempt to the specified endpoint; sends no application request.",
        required_privileges: "None for ordinary destination ports; use only authorized targets.",
        related_diagnostics: &["network.tcp", "security.tls", "web.http"],
        keywords: &[
            "check port",
            "tcp connection",
            "port open",
            "connectivity",
            "nc",
            "netcat",
        ],
    },
    KnowledgeCard {
        id: "windows.tcp-connectivity",
        title: "Check one TCP endpoint",
        operating_system: "Windows PowerShell",
        intent: "Test whether one authorized host accepts a TCP connection on a port.",
        command: "Test-NetConnection -ComputerName example.com -Port 443 -InformationLevel Detailed",
        parameters: "Replace the hostname and port. The cmdlet is included with supported Windows PowerShell environments.",
        example: "Test-NetConnection -ComputerName example.com -Port 443 -InformationLevel Detailed",
        expected_output: "Remote address, route details, and a TcpTestSucceeded true/false value.",
        explanation: "TcpTestSucceeded reports the TCP connection attempt only; it does not validate TLS or application health.",
        risk: "R1 — network connection attempt",
        side_effects: "Sends a connection attempt to the specified endpoint; sends no application request.",
        required_privileges: "None for ordinary destination ports; use only authorized targets.",
        related_diagnostics: &["network.tcp", "security.tls", "web.http"],
        keywords: &[
            "check port",
            "tcp connection",
            "port open",
            "connectivity",
            "Test-NetConnection",
        ],
    },
    KnowledgeCard {
        id: "macos.filesystem-space",
        title: "Check filesystem space",
        operating_system: "macOS",
        intent: "Find mounted filesystems with low available space.",
        command: "df -h",
        parameters: "Optional mount path, such as /System/Volumes/Data.",
        example: "df -h /System/Volumes/Data",
        expected_output: "A table with filesystem, size, used, available, use percentage, and mount point.",
        explanation: "Review the Use% and Available columns to find a volume that is close to full.",
        risk: "R0 — local read-only inspection",
        side_effects: "Does not modify files or filesystem state.",
        required_privileges: "None for mounted filesystems visible to the current user.",
        related_diagnostics: &["system.disk"],
        keywords: &[
            "disk full",
            "free disk space",
            "filesystem",
            "volume",
            "df -h",
            "storage",
        ],
    },
    KnowledgeCard {
        id: "macos.find-large-files",
        title: "Find large files under a directory",
        operating_system: "macOS",
        intent: "Locate files larger than a chosen size without deleting them.",
        command: "find /var/log -type f -size +100M -print",
        parameters: "Replace /var/log with a directory you are authorized to inspect; change 100M to the size threshold.",
        example: "find /var/log -type f -size +100M -print",
        expected_output: "Paths of matching regular files; no output means no matches were found in the searched tree.",
        explanation: "This command only lists matching files. Keep the search root narrow because recursive traversal can take time on large trees.",
        risk: "R1 — read-only directory traversal",
        side_effects: "Reads directory entries and file metadata; does not change or delete files.",
        required_privileges: "Read and traverse permission for the chosen directory.",
        related_diagnostics: &["system.disk", "file.sha256"],
        keywords: &[
            "disk full",
            "large files",
            "log size",
            "find files",
            "100M",
            "macOS",
        ],
    },
    KnowledgeCard {
        id: "macos.dns-lookup",
        title: "Look up host addresses",
        operating_system: "macOS",
        intent: "Query the configured system name service for a hostname.",
        command: "dscacheutil -q host -a name example.com",
        parameters: "Replace example.com with the hostname to inspect.",
        example: "dscacheutil -q host -a name example.com",
        expected_output: "Host records with the name and any returned IPv4 or IPv6 address values.",
        explanation: "The query uses the system Directory Service host lookup and may return cached data or fetch a live result.",
        risk: "R1 — local name-service query",
        side_effects: "May contact configured name services and populate the system-managed lookup cache; does not change resolver configuration.",
        required_privileges: "None for an ordinary host lookup.",
        related_diagnostics: &["network.dns", "network.reverse_dns"],
        keywords: &[
            "dns",
            "resolve hostname",
            "address lookup",
            "dscacheutil",
            "A record",
            "AAAA record",
        ],
    },
    KnowledgeCard {
        id: "macos.tcp-connectivity",
        title: "Check one TCP endpoint",
        operating_system: "macOS",
        intent: "Attempt a bounded TCP connection to one authorized host and port.",
        command: "/usr/bin/nc -vz -G 5 example.com 443",
        parameters: "Replace the hostname and port; -G 5 bounds the TCP connection attempt to five seconds.",
        example: "/usr/bin/nc -vz -G 5 example.com 443",
        expected_output: "A success or failure message from nc; exact wording depends on the macOS version.",
        explanation: "A successful TCP handshake proves only that the endpoint accepted a connection at that moment, not that an application is healthy.",
        risk: "R1 — network connection attempt",
        side_effects: "Sends one connection attempt to the specified endpoint; sends no application request.",
        required_privileges: "None for ordinary destination ports; use only authorized targets.",
        related_diagnostics: &["network.tcp", "security.tls", "web.http"],
        keywords: &[
            "check port",
            "tcp connection",
            "port open",
            "connectivity",
            "netcat",
            "nc",
        ],
    },
    KnowledgeCard {
        id: "solaris.filesystem-space",
        title: "Check filesystem space",
        operating_system: "Solaris",
        intent: "Review mounted filesystems for low available space.",
        command: "df -k",
        parameters: "Optionally provide a mount point or dataset, such as /var; omit it to display all mounted filesystems.",
        example: "df -k /var",
        expected_output: "A table with filesystem, 1024-byte blocks, used and available blocks, capacity, and mount point.",
        explanation: "Review the Available and Capacity columns. Solaris -k output uses 1024-byte blocks; ZFS datasets can appear as separate filesystems.",
        risk: "R0 — local read-only inspection",
        side_effects: "Reads mounted filesystem statistics; does not modify files or filesystem state.",
        required_privileges: "None for mounted filesystems visible to the current user.",
        related_diagnostics: &["system.disk"],
        keywords: &[
            "disk full",
            "free disk space",
            "filesystem",
            "storage",
            "volume",
            "df -k",
        ],
    },
    KnowledgeCard {
        id: "solaris.find-large-files",
        title: "Find large files under a directory",
        operating_system: "Solaris",
        intent: "Locate files larger than a chosen size without deleting them.",
        command: "find /var/adm -type f -size +204800 -print",
        parameters: "Replace /var/adm with an authorized, narrow directory. Solaris find measures -size in 512-byte blocks; 204800 blocks is approximately 100 MiB.",
        example: "find /var/adm -type f -size +204800 -print",
        expected_output: "Paths of regular files larger than the threshold; no output means no matches were found in the searched tree.",
        explanation: "This command only lists matching files. Keep the search root narrow because recursive traversal can take time on large trees.",
        risk: "R1 — read-only directory traversal",
        side_effects: "Reads directory entries and file metadata; does not change or delete files.",
        required_privileges: "Read and traverse permission for the chosen directory.",
        related_diagnostics: &["system.disk", "file.sha256"],
        keywords: &[
            "disk full",
            "large files",
            "log size",
            "find files",
            "204800",
        ],
    },
    KnowledgeCard {
        id: "solaris.dns-lookup",
        title: "Look up a host through the system name service",
        operating_system: "Solaris",
        intent: "Resolve a hostname using the configured hosts name-service sources.",
        command: "getent hosts example.com",
        parameters: "Replace example.com with the hostname to inspect.",
        example: "getent hosts example.com",
        expected_output: "One or more address and hostname rows from the configured hosts database, or no rows when it has no result.",
        explanation: "getent queries the configured hosts name service, which may include DNS, local files, or other configured sources; it does not prove that a DNS server was queried.",
        risk: "R1 — local name-service lookup",
        side_effects: "May contact configured name services for the supplied hostname; does not change resolver configuration.",
        required_privileges: "None for an ordinary host lookup.",
        related_diagnostics: &["network.dns", "network.reverse_dns"],
        keywords: &[
            "dns",
            "resolve hostname",
            "address lookup",
            "getent",
            "hosts database",
        ],
    },
    KnowledgeCard {
        id: "solaris.tcp-connectivity",
        title: "Check one TCP endpoint",
        operating_system: "Solaris",
        intent: "Attempt one TCP connection to an explicitly authorized host and port.",
        command: "nc -z host.example.com 443",
        parameters: "Replace the hostname and port. Requires the Solaris network/netcat package, which may not be installed. This command does not set a separate connection timeout.",
        example: "nc -z host.example.com 443",
        expected_output: "A success message for an open TCP port or a connection diagnostic; exact wording depends on the installed Solaris version.",
        explanation: "The -z mode attempts a TCP connection without sending application data. Use it only for an authorized single endpoint; filtered connections can wait for the OS TCP stack to time out. NYRA's typed TCP check provides a bounded timeout when available.",
        risk: "R1 — single authorized network connection attempt",
        side_effects: "Sends a TCP connection attempt to the specified endpoint and sends no application payload.",
        required_privileges: "None for an outbound connection to an ordinary destination port.",
        related_diagnostics: &["network.tcp", "security.tls", "web.http"],
        keywords: &[
            "check port",
            "tcp connection",
            "port open",
            "connectivity",
            "nc",
            "netcat",
        ],
    },
    KnowledgeCard {
        id: "solaris.local-tcp-sockets",
        title: "Inspect local TCP sockets",
        operating_system: "Solaris",
        intent: "Review local TCP socket states, including listening sockets.",
        command: "netstat -an -P tcp",
        parameters: "No required parameters. The -a option includes connected and unconnected sockets; -n displays numeric addresses and ports.",
        example: "netstat -an -P tcp",
        expected_output: "TCP socket rows with local and remote addresses, queues, and state such as LISTEN or ESTABLISHED.",
        explanation: "This inspects TCP sockets visible on the current Solaris host. It does not probe or verify a remote endpoint.",
        risk: "R0 — local read-only inspection",
        side_effects: "Displays local and peer addresses and ports; reads local network status without contacting remote hosts or changing socket state.",
        required_privileges: "None for ordinary socket status visible in the current zone.",
        related_diagnostics: &["network.tcp", "network.interfaces"],
        keywords: &[
            "listening port",
            "active tcp sockets",
            "netstat",
            "socket state",
            "local ports",
        ],
    },
    KnowledgeCard {
        id: "freebsd.filesystem-capacity",
        title: "Inspect local filesystem capacity and inodes",
        operating_system: "FreeBSD",
        intent: "Review used and available filesystem space plus inode usage on a local mount.",
        command: "df -h -i -l /var",
        parameters: "Replace /var with a path on a locally mounted filesystem. The -l option excludes nonlocal mounts.",
        example: "df -h -i -l /var",
        expected_output: "Filesystem capacity and human-readable size columns, plus used/free inode counts and inode-use percentage. Filesystems without inodes may show a dash.",
        explanation: "FreeBSD df(1) documents -h for human-readable sizes and -i for free/used inode statistics; -l selects locally mounted filesystems. See the official manual: https://man.freebsd.org/cgi/man.cgi?query=df&sektion=1. NYRA displays or copies this command only; it does not execute it.",
        risk: "R0 — local read-only inspection",
        side_effects: "Reads mounted filesystem statistics; does not modify files or filesystem state.",
        required_privileges: "None for a mounted filesystem visible to the current user.",
        related_diagnostics: &["system.disk"],
        keywords: &[
            "freebsd",
            "disk full",
            "free disk space",
            "filesystem",
            "inodes",
            "df",
            "storage",
        ],
    },
    KnowledgeCard {
        id: "freebsd.local-tcp-sockets",
        title: "Inspect local listening TCP sockets",
        operating_system: "FreeBSD",
        intent: "Review IPv4 and IPv6 TCP sockets that are listening on the local host.",
        command: "sockstat -4 -6 -l -P tcp",
        parameters: "No required parameters. A command run inside a jail only sees sockets available in that jail; permissions can also limit process details.",
        example: "sockstat -4 -6 -l -P tcp",
        expected_output: "Rows with the visible owning user, command, process ID, file descriptor, protocol, and local/foreign addresses for listening TCP sockets.",
        explanation: "FreeBSD sockstat(1) documents -4/-6 for IPv4/IPv6, -l for listening sockets, and -P for protocol filtering. The FreeBSD Handbook notes that a process inside a jail sees only that jail's sockets, not the host or other jails. See the official manual: https://man.freebsd.org/cgi/man.cgi?query=sockstat&sektion=1 and Handbook: https://docs.freebsd.org/en/books/handbook/jails/. NYRA displays or copies this command only; it does not execute it.",
        risk: "R0 — local read-only inspection",
        side_effects: "Displays local socket, process, user, and port metadata; sends no network traffic and does not change socket state.",
        required_privileges: "None for sockets visible to the current user; visibility depends on permissions and jail scope.",
        related_diagnostics: &["network.tcp", "network.interfaces"],
        keywords: &[
            "freebsd",
            "listening ports",
            "local tcp sockets",
            "tcp socket status",
            "sockstat",
            "local ports",
        ],
    },
];

pub fn find_knowledge_card(id: &str) -> Option<&'static KnowledgeCard> {
    KNOWLEDGE_CARDS.iter().find(|card| card.id == id)
}

pub fn search_knowledge_cards(query: &str) -> Vec<&'static KnowledgeCard> {
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return Vec::new();
    }
    let terms = query.split_whitespace().collect::<Vec<_>>();
    KNOWLEDGE_CARDS
        .iter()
        .filter(|card| {
            let searchable = format!(
                "{} {} {} {} {} {}",
                card.id,
                card.title,
                card.operating_system,
                card.intent,
                card.command,
                card.keywords.join(" ")
            )
            .to_lowercase();
            terms.iter().all(|term| searchable.contains(term))
        })
        .collect()
}

/// Returns local knowledge matches in relevance order, keeping catalog order for ties.
/// This matcher is intentionally limited to curated card metadata; it is not a language model.
pub fn rank_knowledge_cards(query: &str) -> Vec<&'static KnowledgeCard> {
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return KNOWLEDGE_CARDS.iter().collect();
    }
    let terms = query.split_whitespace().collect::<Vec<_>>();
    let mut matches = KNOWLEDGE_CARDS
        .iter()
        .enumerate()
        .filter_map(|(index, card)| {
            knowledge_match_score(card, &query, &terms).map(|score| (score, index, card))
        })
        .collect::<Vec<_>>();
    matches.sort_by_key(|(score, index, _)| (*score, *index));
    matches.into_iter().map(|(_, _, card)| card).collect()
}

fn knowledge_match_score(card: &KnowledgeCard, query: &str, terms: &[&str]) -> Option<u8> {
    let keywords = card
        .keywords
        .iter()
        .map(|keyword| keyword.to_lowercase())
        .collect::<Vec<_>>();
    let fields = [
        card.id.to_lowercase(),
        card.title.to_lowercase(),
        card.operating_system.to_lowercase(),
        card.intent.to_lowercase(),
        card.command.to_lowercase(),
    ];
    let searchable = fields
        .iter()
        .chain(keywords.iter())
        .map(String::as_str)
        .collect::<Vec<_>>();

    if searchable.contains(&query) {
        return Some(0);
    }
    if keywords.iter().any(|keyword| keyword == query) {
        return Some(1);
    }
    if card.title.to_lowercase() == query {
        return Some(2);
    }
    if searchable.iter().any(|field| field.contains(query)) {
        return Some(3);
    }
    if terms
        .iter()
        .all(|term| searchable.iter().any(|field| field.contains(term)))
    {
        return Some(4);
    }

    let words = searchable
        .iter()
        .flat_map(|field| field.split(|character: char| !character.is_alphanumeric()))
        .filter(|word| !word.is_empty())
        .collect::<Vec<_>>();
    terms
        .iter()
        .all(|term| {
            words.iter().any(|word| {
                within_edit_distance(term, word, if term.chars().count() <= 4 { 1 } else { 2 })
            })
        })
        .then_some(5)
}

fn within_edit_distance(left: &str, right: &str, maximum: usize) -> bool {
    let left = left.chars().collect::<Vec<_>>();
    let right = right.chars().collect::<Vec<_>>();
    if left.len().abs_diff(right.len()) > maximum {
        return false;
    }

    let mut previous = (0..=right.len()).collect::<Vec<_>>();
    let mut current = vec![0; right.len() + 1];
    for (left_index, left_character) in left.iter().enumerate() {
        current[0] = left_index + 1;
        for (right_index, right_character) in right.iter().enumerate() {
            current[right_index + 1] = (previous[right_index + 1] + 1)
                .min(current[right_index] + 1)
                .min(previous[right_index] + usize::from(left_character != right_character));
        }
        std::mem::swap(&mut previous, &mut current);
    }
    previous[right.len()] <= maximum
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn cards_have_unique_ids_and_complete_read_only_guidance() {
        let mut ids = HashSet::new();
        for card in KNOWLEDGE_CARDS {
            assert!(ids.insert(card.id), "duplicate card id: {}", card.id);
            assert!(!card.title.is_empty(), "missing title for {}", card.id);
            assert!(!card.command.is_empty(), "missing command for {}", card.id);
            assert!(!card.example.is_empty(), "missing example for {}", card.id);
            assert!(!card.risk.is_empty(), "missing risk for {}", card.id);
            assert!(
                !card.side_effects.is_empty(),
                "missing side effects for {}",
                card.id
            );
            assert!(
                !card.related_diagnostics.is_empty(),
                "missing related diagnostics for {}",
                card.id
            );
        }
    }

    #[test]
    fn includes_macos_cards_for_the_initial_storage_and_network_checks() {
        let macos_cards = KNOWLEDGE_CARDS
            .iter()
            .filter(|card| card.operating_system == "macOS")
            .collect::<Vec<_>>();
        assert_eq!(macos_cards.len(), 4);
        assert!(
            macos_cards
                .iter()
                .any(|card| card.id == "macos.filesystem-space")
        );
        assert!(
            macos_cards
                .iter()
                .any(|card| card.id == "macos.find-large-files")
        );
        assert!(macos_cards.iter().any(|card| card.id == "macos.dns-lookup"));
        assert!(
            macos_cards
                .iter()
                .any(|card| card.id == "macos.tcp-connectivity")
        );
    }

    #[test]
    fn includes_read_only_solaris_cards_for_storage_name_service_and_network_status() {
        let solaris_cards = KNOWLEDGE_CARDS
            .iter()
            .filter(|card| card.operating_system == "Solaris")
            .collect::<Vec<_>>();
        assert_eq!(solaris_cards.len(), 5);
        for id in [
            "solaris.filesystem-space",
            "solaris.find-large-files",
            "solaris.dns-lookup",
            "solaris.tcp-connectivity",
            "solaris.local-tcp-sockets",
        ] {
            assert!(
                solaris_cards.iter().any(|card| card.id == id),
                "missing {id}"
            );
        }
        assert_eq!(
            find_knowledge_card("solaris.find-large-files")
                .unwrap()
                .command,
            "find /var/adm -type f -size +204800 -print"
        );
        assert_eq!(
            find_knowledge_card("solaris.local-tcp-sockets")
                .unwrap()
                .command,
            "netstat -an -P tcp"
        );
        let tcp_card = find_knowledge_card("solaris.tcp-connectivity").unwrap();
        assert!(
            tcp_card
                .parameters
                .contains("does not set a separate connection timeout")
        );
        assert!(tcp_card.side_effects.contains("no application payload"));
        assert!(
            find_knowledge_card("solaris.local-tcp-sockets")
                .unwrap()
                .explanation
                .contains("does not probe or verify a remote endpoint")
        );
    }

    #[test]
    fn includes_read_only_freebsd_filesystem_and_socket_cards_with_official_references() {
        let freebsd_cards = KNOWLEDGE_CARDS
            .iter()
            .filter(|card| card.operating_system == "FreeBSD")
            .collect::<Vec<_>>();
        assert_eq!(freebsd_cards.len(), 2);

        let filesystem = find_knowledge_card("freebsd.filesystem-capacity").unwrap();
        assert_eq!(filesystem.command, "df -h -i -l /var");
        assert!(
            filesystem
                .explanation
                .contains("man.freebsd.org/cgi/man.cgi?query=df")
        );

        let sockets = find_knowledge_card("freebsd.local-tcp-sockets").unwrap();
        assert_eq!(sockets.command, "sockstat -4 -6 -l -P tcp");
        assert!(
            sockets
                .explanation
                .contains("man.freebsd.org/cgi/man.cgi?query=sockstat")
        );
        assert!(
            sockets
                .explanation
                .contains("docs.freebsd.org/en/books/handbook/jails")
        );
        assert!(sockets.parameters.contains("inside a jail"));
        assert!(sockets.side_effects.contains("sends no network traffic"));

        for card in [filesystem, sockets] {
            assert!(card.explanation.contains("does not execute it"));
            assert_eq!(card.risk, "R0 — local read-only inspection");
        }
        assert_eq!(
            search_knowledge_cards("freebsd filesystem inodes")[0].id,
            "freebsd.filesystem-capacity"
        );
        assert_eq!(
            search_knowledge_cards("freebsd listening tcp sockets")[0].id,
            "freebsd.local-tcp-sockets"
        );
    }

    #[test]
    fn finds_cards_by_stable_id() {
        assert_eq!(
            find_knowledge_card("linux.tcp-connectivity").unwrap().title,
            "Check one TCP endpoint"
        );
        assert!(find_knowledge_card("unknown.card").is_none());
    }

    #[test]
    fn searches_cards_by_all_query_terms_in_stable_catalog_order() {
        let cards = search_knowledge_cards("check port");
        assert_eq!(cards.len(), 4);
        assert_eq!(cards[0].id, "linux.tcp-connectivity");
        assert_eq!(cards[1].id, "windows.tcp-connectivity");
        assert_eq!(cards[2].id, "macos.tcp-connectivity");
        assert_eq!(cards[3].id, "solaris.tcp-connectivity");
        assert!(search_knowledge_cards("no matching recipe").is_empty());
    }

    #[test]
    fn ranking_prefers_direct_matches_and_recovers_curated_search_typos() {
        let exact = rank_knowledge_cards("check one tcp endpoint");
        assert_eq!(exact[0].id, "linux.tcp-connectivity");

        let typo = rank_knowledge_cards("tcp connectvity");
        assert_eq!(typo.len(), 4);
        assert_eq!(typo[0].id, "linux.tcp-connectivity");
        assert_eq!(typo[1].id, "windows.tcp-connectivity");
        assert_eq!(typo[2].id, "macos.tcp-connectivity");
        assert_eq!(typo[3].id, "solaris.tcp-connectivity");
    }
}
