# NYRA — สรุปโปรเจกต์ภาษาไทย (Project Brief)

> **สถานะ:** วางแผนและจัดทำเอกสารแล้ว / ยังไม่ได้เริ่มเขียนหรือทดสอบโปรแกรมจริง
>
> **ชื่อ:** NYRA (ชื่อเดิมชั่วคราว PocketOps)
>
> **คำโปรย:** Your System. Your Tools. Anywhere.
>
> **วัตถุประสงค์:** Toolkit ส่วนตัวสำหรับ System Administrator พกพาผ่าน USB หรือดาวน์โหลดจาก GitHub แล้วใช้งานตรวจสอบระบบได้ทันทีบน OS ที่รองรับ โดยไม่ต้องจำคำสั่ง และไม่ต้องติดตั้งเครื่องมือพ่วงมากมาย

## 1. ทำไมต้องสร้าง NYRA
ปัญหาที่ผู้ใช้เจอ: พอไปถึงเครื่องที่ไม่ใช่เครื่องตัวเอง เครื่องมืออย่าง Nmap อาจไม่ได้ติดตั้ง; หรือจำไม่ได้ว่าต้องใช้คำสั่งอะไรเพื่อตรวจ Port, Network, Disk, Certificate, Service หรือ Database ต้องกลับไปถาม AI/ค้นเว็บทุกครั้ง จึงต้องการโปรแกรมเดียวที่ค้นหาจาก **สิ่งที่ต้องการทำ** และใช้งานได้ง่าย รวดเร็วจริง

## 2. 3 รูปแบบการใช้งาน
1. **Interactive TUI:** พิมพ์ \`nyra\` เพื่อเข้าหน้าจอ Terminal UI มี Search, หมวด Tools, ฟอร์มกรอก IP/Port, ผลการตรวจ, Favorites และ Settings ใช้คีย์บอร์ดได้ทั้งระบบ
2. **Direct CLI:** เช่น \`nyra net tcp 192.168.1.10 443\` ให้ผลลัพธ์ตรง ๆ ใช้งานใน Script และ Automation ได้ รองรับ JSON และ Exit Code ที่ชัดเจน
3. **Workflow Mode:** เช่น \`nyra workflow run web-unreachable --target https://example.com\` เพื่อตรวจ DNS → TCP → TLS → HTTP และอธิบายผลตรวจแต่ละขั้น

> **หมายเหตุ:** คำสั่งเหล่านี้เป็นรูปแบบที่ตกลงจะพัฒนา **ยังไม่ใช่คำสั่งโปรแกรมที่ใช้งานได้แล้ว**

## 3. หน้าตาและ UX
แรงบันดาลใจจาก Capri: terminal แนว Cyberpunk ดูเท่ มีบุคลิก เสียงและลูกเล่น แต่ต้องสร้างด้วย Rust + Ratatui ใน Terminal จริง ไม่ใช่ภาพ UI ที่เขียนตามไม่ได้

หน้าจอหลัก: Header → Search → Categories/Tools/Detail Form → Result Viewer → Shortcut Footer โดยปรับตามขนาดหน้าจอ (3/2/1 คอลัมน์) และยังใช้งานได้เมื่อ Terminal เล็ก ออกแบบสำหรับ Keyboard-first, ไม่ต้องจำคำสั่ง, ใช้คำอธิบายภาษาอังกฤษ

## 4. เรื่องสี — ตกลงไว้ชัดเจนแล้ว
**ไม่เลือกธีมเดียว:** ต้องมี **4 ธีม** ได้แก่
- Cyber Cyan — Cyan / Magenta / Navy
- Neo Tokyo — Purple / Pink / Orange
- Acid Grid — Lime / Cyan / Yellow
- Crimson Noir — Crimson / Violet / Black

ต้องเปลี่ยนธีมใน Settings ได้ทันทีโดยไม่ Restart, ปรับ HEX ของสีต่าง ๆ เองได้, นำเข้า/ส่งออก Theme และเก็บ Config พกไปกับ USB ได้

ไฟล์ Preview (Editable SVG) อยู่ใน [assets/mockups/](../assets/mockups/). อ่านรายละเอียดใน [Experience Spec](experience.md)

## 5. เสียงและ Animation
มี Startup Boot, Menu Select, Run Tool, Success, Warning, Error, Workflow Complete พร้อม Volume / Mute / ปิดรายเหตุการณ์ได้
มี Typing/Scan/Selection/Glitch Effects แบบที่ Terminal ทำได้จริง ต้องไม่ทำให้โปรแกรมช้า และปิด Animation ได้ทั้งหมด
ไม่มี Audio Device หรือใช้ SSH/Headless ก็ยังรัน NYRA ได้ตามปกติ

## 6. Tools ที่จะพัฒนา
**ระยะแรก 12 ตัว**: TCP Port Check, DNS Lookup, Reverse DNS, HTTP Health, TLS Inspection, Local Network Info, Ping, System Info, CPU, Memory, Disk และ File SHA-256

**ระยะถัดไป**: Traceroute, Multi-Port, Nmap (Optional), Process/Service/Listening Port, Log Search/Tail/Parser, Security Headers, Oracle Listener/DB Health, MSSQL, SSH/SFTP Check, Remote Linux/Solaris/Windows

หลักคือ **Native Rust ก่อน** หากจำเป็นจึงใช้ OS/External/Remote Adapter ตรวจพบเครื่องมือที่ไม่มีแล้วต้องแจ้งชัด ห้ามสัญญาว่าฟังก์ชันที่ต้องใช้ Npcap, System Privileges หรือ Library ภายนอกจะ Zero-install ทุกเครื่อง

## 7. Command Library และการค้นหา
ผู้ใช้พิมพ์คำอย่าง "เช็คพอร์ต", "ดิสก์เต็ม", "cert หมดอายุ", "oracle listener" แล้วหา Tool/คำสั่งที่เกี่ยวข้องได้ มีคำอธิบาย Syntax, ตัวอย่าง, ความเสี่ยง, ปุ่ม Copy และ Input Form
ช่วงแรกใช้ Keywords + English Aliases + Fuzzy Search แบบ Offline ก่อน ไม่ต้องใช้ AI และไม่ต้องเสีย API Token

## 8. Workflows และ Incident Mode
Workflow ตรวจ Web, Basic Server Health, Database Connectivity, Certificate, SFTP และ Linux Disk
Incident Mode ให้เลือกอาการ เช่น เว็บล่ม, Connection Timeout, Server Slow, Disk Full, Certificate Problem เพื่อให้ระบบพาตรวจแบบเป็นขั้นตอนและสรุปจากหลักฐานที่เจอจริง
เวอร์ชันแรกไม่ต้องมีระบบแก้ไขระบบอัตโนมัติ ใช้ Read-only เป็นหลัก

## 9. Portable
- ดาวน์โหลด Binary ที่ Build ไว้จาก GitHub Releases สำหรับ Windows/Linux ที่ทดสอบจริง
- รันจาก Flash Drive ได้ ใช้ \`portable.flag\` เพื่อเก็บ Settings, Themes, Profiles, Favorites, History/Reports ไว้ข้างโปรแกรม
- เครื่องปลายทางไม่ต้องติดตั้ง Rust/Python
- ต้องแยก Build สำหรับ OS/CPU; โปรแกรมเดียวไม่สามารถรันทุก OS ได้
- Windows 11 / Server 2022 / Ubuntu / RHEL เป็นเป้าหมายหลัก; Solaris 11.4 SPARC เน้น Remote SSH; Windows Server 2008 R2 ไม่รับประกัน Native
- ถ้า USB เขียนไม่ได้ให้ใช้งาน Read-only Tool ได้ และแจ้งชัดว่าบันทึกไม่ได้

## 10. Security
เริ่มต้น Read-only, Least Privilege, ใช้ Process Args ที่ผ่าน Validation ไม่ต่อ Shell String จาก Input, จำกัดจำนวนงาน/Timeout, Remote SSH ต้องตรวจ Host Key, ไม่เก็บ Password/Token เป็น Plaintext, Redact Data ก่อน Export/History, ไม่ข้ามนโยบายความปลอดภัยของเครื่องเป้าหมาย

## 11. โครงสร้างทางเทคนิค
Rust Stable + Clap (CLI) + Ratatui/Crossterm (TUI) + Tokio (Async) + Tool Registry/Executor ตัวเดียวกัน + Serde/TOML Config
แบ่งเป็น Modular Monolith; Native Tools / OS Adapters / Remote Adapters / Workflow Engine / Reporting / Knowledge Search / Theme & Audio Engine
เก็บ Source บน GitHub Repo นี้ให้ Codex Cloud ทำต่อ และบันทึกสถานะการทดสอบจริงทุกระยะ

## 12. Roadmap ตั้งแต่ต้นจนจบ
| Phase | งาน |
|---|---|
| 0 | PRD, UX, Architecture, CLI Contracts, Test Plan |
| 1 | Rust Project + CLI + Tool Registry + Executor + Tests |
| 2 | Native Diagnostics อย่างน้อย 12 Tools |
| 3 | Interactive Ratatui TUI |
| 4 | 4 Themes + Custom Colors + Sound/Animation |
| 5 | Search, Knowledge Base, Command Recipes |
| 6 | Workflow Engine + Reports |
| 7 | Portable Build + GitHub Release + Clean VM/USB Tests |
| 8 | Remote Linux/Solaris/Windows + Oracle/MSSQL + Optional Nmap |
| 9 | Incident Mode + Security/UX/Performance Hardening + v1.0 |

**แนะนำให้เริ่ม Phase 1 แบบ Vertical Slice**: \`nyra --help\`, \`nyra tools list\` และ \`nyra net tcp\` ให้ใช้งาน/ทดสอบได้จริงก่อนขยายเป็น 100 Tools

## 13. AI Agent Skills
ชุดที่เสนอ: AI Hero Main Flow, rust-expert, rust-cli-clap, rust-tui-ratatui, agent-cli-design, code-security, technical-writing, semgrep และ Diagram Design Skill
ติดตั้งหลังตรวจ Source/SKILL.md/สคริปต์และความเข้ากันได้ของ Codex Cloud; ยังไม่ได้ติดตั้งใน Repo นี้ อ่าน [Agent Skills](agent-skills.md) และ [AGENTS.md](../AGENTS.md)

**ข้อจำกัด:** Diagram Design Skill ที่ผู้ใช้ต้องการ **ไม่พร้อมใช้งานในเซสชันที่เขียนเอกสารนี้** จึงยังไม่ได้สร้าง Architecture Diagram ห้ามอ้างว่าใช้ Skill แล้ว ภายหลังเมื่อพร้อมต้องสร้างภาพจริงใน \`assets/diagrams/\` พร้อมแหล่งไฟล์แก้ไขได้ถ้ารองรับและลิงก์ Relative Path ที่ตรวจสอบได้

## 14. ลิงก์เริ่มทำกับ Codex Cloud
เริ่มจาก [Codex Start Here](CODEX_START_HERE.md), ตามด้วย [NYRA-001 งานแรก](CODEX_TASK_001.md) และ [Roadmap](roadmap.md)
คำสั่งภาษาอังกฤษที่ใช้เริ่มได้:

> Read AGENTS.md and docs/CODEX_START_HERE.md. Implement docs/CODEX_TASK_001.md as a small Rust vertical slice. Do not assume anything is implemented. Run real tests, update status honestly, and preserve all locked product decisions.

## สถานะปัจจุบัน
✅ มีข้อตกลง/สเปกและเอกสารส่งต่อ Codex Cloud แล้ว
✅ มี SVG Mockup ทั้ง 4 Themes เป็น Visual Reference ใน GitHub
⏳ ยังไม่มี Cargo Project / แอป NYRA ที่รันได้
⏳ ยังไม่ได้ติดตั้ง Skills, สร้าง Architecture Diagram หรือรันทดสอบ OS
⏳ ภาพ PNG ต้นฉบับจากการสร้างภาพในแชตก่อนหน้าไม่ได้ Commit เป็น Binary ใน Repository; SVG ใน Repo เป็นภาพแบบใหม่ที่แก้ไขได้

สุดท้ายให้วัดความสำเร็จจาก **การใช้แก้ปัญหา IT ได้เร็วขึ้นจริง** ไม่ใช่จำนวน Tools หรือความสวยอย่างเดียว
