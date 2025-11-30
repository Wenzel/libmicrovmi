// Populate the sidebar
//
// This is a script, and not included directly in the page, to control the total size of the book.
// The TOC contains an entry for each page, so if each page includes a copy of the TOC,
// the total size of the page becomes O(n**2).
class MDBookSidebarScrollbox extends HTMLElement {
    constructor() {
        super();
    }
    connectedCallback() {
        this.innerHTML = '<ol class="chapter"><li class="chapter-item expanded "><a href="index.html"><strong aria-hidden="true">1.</strong> Introduction</a></li><li class="chapter-item expanded affix "><li class="part-title">Tutorials</li><li class="chapter-item expanded "><a href="tutorial/installation.html"><strong aria-hidden="true">2.</strong> Installation</a></li><li class="chapter-item expanded "><a href="tutorial/memprocfs_qemu.html"><strong aria-hidden="true">3.</strong> Listing Windows 10 Services using MemProcFS on QEMU (Linux)</a></li><li class="chapter-item expanded "><a href="tutorial/volatility3_xen.html"><strong aria-hidden="true">4.</strong> Listing Windows 10 Processes using Volatility3 on Xen</a></li><li class="chapter-item expanded "><a href="tutorial/libvmi.html"><strong aria-hidden="true">5.</strong> Run LibVMI fork on memflow</a></li><li class="chapter-item expanded affix "><li class="part-title">Reference</li><li class="chapter-item expanded "><a href="reference/integration.html"><strong aria-hidden="true">6.</strong> Integration</a></li><li><ol class="section"><li class="chapter-item expanded "><a href="reference/integration/libvmi.html"><strong aria-hidden="true">6.1.</strong> LibVMI</a></li><li class="chapter-item expanded "><a href="reference/integration/volatility3.html"><strong aria-hidden="true">6.2.</strong> volatility3</a></li><li class="chapter-item expanded "><a href="reference/integration/leechcore.html"><strong aria-hidden="true">6.3.</strong> LeechCore</a></li></ol></li><li class="chapter-item expanded "><a href="reference/drivers.html"><strong aria-hidden="true">7.</strong> Drivers</a></li><li><ol class="section"><li class="chapter-item expanded "><a href="reference/drivers/xen.html"><strong aria-hidden="true">7.1.</strong> Xen</a></li><li class="chapter-item expanded "><a href="reference/drivers/kvm.html"><strong aria-hidden="true">7.2.</strong> KVM</a></li><li class="chapter-item expanded "><a href="reference/drivers/virtualbox.html"><strong aria-hidden="true">7.3.</strong> VirtualBox</a></li><li class="chapter-item expanded "><a href="reference/drivers/memflow.html"><strong aria-hidden="true">7.4.</strong> memflow</a></li></ol></li><li class="chapter-item expanded "><a href="reference/api.html"><strong aria-hidden="true">8.</strong> API</a></li><li><ol class="section"><li class="chapter-item expanded "><a href="reference/api/rust_api.html"><strong aria-hidden="true">8.1.</strong> Rust API</a></li><li class="chapter-item expanded "><a href="reference/api/python_api.html"><strong aria-hidden="true">8.2.</strong> Python API</a></li><li class="chapter-item expanded "><a href="reference/api/c_api.html"><strong aria-hidden="true">8.3.</strong> C API</a></li></ol></li><li class="chapter-item expanded "><li class="part-title">Explanation</li><li class="chapter-item expanded "><a href="explanation/vmi_api.html"><strong aria-hidden="true">9.</strong> VMI API</a></li><li class="chapter-item expanded "><a href="explanation/vmi_ecosystem.html"><strong aria-hidden="true">10.</strong> VMI Fragmentation</a></li><li class="chapter-item expanded affix "><li class="part-title">Developer</li><li class="chapter-item expanded "><a href="developer/libmicrovmi.html"><strong aria-hidden="true">11.</strong> libmicrovmi</a></li><li class="chapter-item expanded "><a href="developer/source.html"><strong aria-hidden="true">12.</strong> Source installation</a></li><li class="chapter-item expanded "><a href="developer/intro_mem_dump.html"><strong aria-hidden="true">13.</strong> Memory dump example on Xen</a></li><li><ol class="section"><li class="chapter-item expanded "><a href="developer/mem_dump/rust.html"><strong aria-hidden="true">13.1.</strong> Rust</a></li><li class="chapter-item expanded "><a href="developer/mem_dump/c.html"><strong aria-hidden="true">13.2.</strong> C</a></li><li class="chapter-item expanded "><a href="developer/mem_dump/python.html"><strong aria-hidden="true">13.3.</strong> Python</a></li></ol></li><li class="chapter-item expanded "><a href="developer/python.html"><strong aria-hidden="true">14.</strong> Python</a></li><li class="chapter-item expanded "><a href="developer/tests.html"><strong aria-hidden="true">15.</strong> Tests</a></li><li class="chapter-item expanded "><a href="developer/release.html"><strong aria-hidden="true">16.</strong> Release</a></li><li class="chapter-item expanded "><a href="developer/credits.html"><strong aria-hidden="true">17.</strong> Credits</a></li></ol>';
        // Set the current, active page, and reveal it if it's hidden
        let current_page = document.location.href.toString().split("#")[0].split("?")[0];
        if (current_page.endsWith("/")) {
            current_page += "index.html";
        }
        var links = Array.prototype.slice.call(this.querySelectorAll("a"));
        var l = links.length;
        for (var i = 0; i < l; ++i) {
            var link = links[i];
            var href = link.getAttribute("href");
            if (href && !href.startsWith("#") && !/^(?:[a-z+]+:)?\/\//.test(href)) {
                link.href = path_to_root + href;
            }
            // The "index" page is supposed to alias the first chapter in the book.
            if (link.href === current_page || (i === 0 && path_to_root === "" && current_page.endsWith("/index.html"))) {
                link.classList.add("active");
                var parent = link.parentElement;
                if (parent && parent.classList.contains("chapter-item")) {
                    parent.classList.add("expanded");
                }
                while (parent) {
                    if (parent.tagName === "LI" && parent.previousElementSibling) {
                        if (parent.previousElementSibling.classList.contains("chapter-item")) {
                            parent.previousElementSibling.classList.add("expanded");
                        }
                    }
                    parent = parent.parentElement;
                }
            }
        }
        // Track and set sidebar scroll position
        this.addEventListener('click', function(e) {
            if (e.target.tagName === 'A') {
                sessionStorage.setItem('sidebar-scroll', this.scrollTop);
            }
        }, { passive: true });
        var sidebarScrollTop = sessionStorage.getItem('sidebar-scroll');
        sessionStorage.removeItem('sidebar-scroll');
        if (sidebarScrollTop) {
            // preserve sidebar scroll position when navigating via links within sidebar
            this.scrollTop = sidebarScrollTop;
        } else {
            // scroll sidebar to current active section when navigating via "next/previous chapter" buttons
            var activeSection = document.querySelector('#sidebar .active');
            if (activeSection) {
                activeSection.scrollIntoView({ block: 'center' });
            }
        }
        // Toggle buttons
        var sidebarAnchorToggles = document.querySelectorAll('#sidebar a.toggle');
        function toggleSection(ev) {
            ev.currentTarget.parentElement.classList.toggle('expanded');
        }
        Array.from(sidebarAnchorToggles).forEach(function (el) {
            el.addEventListener('click', toggleSection);
        });
    }
}
window.customElements.define("mdbook-sidebar-scrollbox", MDBookSidebarScrollbox);
