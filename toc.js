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
        this.innerHTML = '<ol class="chapter"><li class="chapter-item expanded affix "><a href="intro.html">Introduction</a></li><li class="chapter-item expanded affix "><li class="part-title">Getting Started</li><li class="chapter-item expanded "><a href="getting_started/installation/index.html"><strong aria-hidden="true">1.</strong> Installation</a></li><li><ol class="section"><li class="chapter-item expanded "><div><strong aria-hidden="true">1.1.</strong> Downloading a Nightly Release</div></li><li class="chapter-item expanded "><div><strong aria-hidden="true">1.2.</strong> Building from Source</div></li></ol></li><li class="chapter-item expanded "><div><strong aria-hidden="true">2.</strong> Command Line Guide</div></li><li><ol class="section"><li class="chapter-item expanded "><a href="getting_started/command-line/logging.html"><strong aria-hidden="true">2.1.</strong> Logging</a></li></ol></li><li class="chapter-item expanded "><li class="part-title">User Guide</li><li class="chapter-item expanded "><div><strong aria-hidden="true">3.</strong> Reading the docs</div></li><li class="chapter-item expanded "><div><strong aria-hidden="true">4.</strong> Useful Tutorials</div></li><li class="chapter-item expanded affix "><li class="part-title">Developers Guide</li><li class="chapter-item expanded "><a href="developers_guide/contributors-guide/index.html"><strong aria-hidden="true">5.</strong> Contributor&#39;s Guide</a></li><li><ol class="section"><li class="chapter-item expanded "><a href="developers_guide/contributors-guide/how_we_work.html"><strong aria-hidden="true">5.1.</strong> How we work</a></li><li class="chapter-item expanded "><a href="developers_guide/contributors-guide/set-dev-env.html"><strong aria-hidden="true">5.2.</strong> Setting up your development environment</a></li><li class="chapter-item expanded "><div><strong aria-hidden="true">5.3.</strong> Running and writing integration tests</div></li><li class="chapter-item expanded "><div><strong aria-hidden="true">5.4.</strong> What we didn&#39;t do</div></li></ol></li><li class="chapter-item expanded "><a href="developers_guide/coding_resources/index.html"><strong aria-hidden="true">6.</strong> Coding Resources and Conventions</a></li><li><ol class="section"><li class="chapter-item expanded "><a href="developers_guide/coding_resources/style_guide.html"><strong aria-hidden="true">6.1.</strong> Style Guide</a></li><li class="chapter-item expanded "><a href="developers_guide/coding_resources/crate_structure.html"><strong aria-hidden="true">6.2.</strong> Crate Structure</a></li></ol></li><li class="chapter-item expanded "><a href="developers_guide/design_documents/index.html"><strong aria-hidden="true">7.</strong> Design Documents</a></li><li><ol class="section"><li class="chapter-item expanded "><a href="developers_guide/design_documents/2023_11.html"><strong aria-hidden="true">7.1.</strong> 2023‐11: High Level Plan</a></li><li class="chapter-item expanded "><a href="developers_guide/design_documents/2024_03.html"><strong aria-hidden="true">7.2.</strong> 2024‐03: Implementing Uniplates and Biplates with Structure Preserving Trees</a></li><li class="chapter-item expanded "><a href="developers_guide/design_documents/expression_rewriting.html"><strong aria-hidden="true">7.3.</strong> Expression rewriting, Rules and RuleSets</a></li><li class="chapter-item expanded "><a href="developers_guide/design_documents/semantics-of-rewriting-expressions.html"><strong aria-hidden="true">7.4.</strong> Semantics of Rewriting Expressions with Side‐Effects</a></li><li class="chapter-item expanded "><a href="developers_guide/design_documents/ideal-scenario-of-testing.html"><strong aria-hidden="true">7.5.</strong> Ideal Scenario of Testing for Conjure-Oxide</a></li></ol></li><li class="chapter-item expanded "><a href="developers_guide/ci_cd/index.html"><strong aria-hidden="true">8.</strong> CI/CD</a></li><li><ol class="section"><li class="chapter-item expanded "><a href="developers_guide/ci_cd/coverage.html"><strong aria-hidden="true">8.1.</strong> Coverage</a></li><li class="chapter-item expanded "><a href="developers_guide/ci_cd/github_actions.html"><strong aria-hidden="true">8.2.</strong> Github Actions: Cookbook</a></li></ol></li><li class="chapter-item expanded "><div><strong aria-hidden="true">9.</strong> Reading the docs</div></li><li class="chapter-item expanded "><div><strong aria-hidden="true">10.</strong> Useful Resources</div></li><li class="chapter-item expanded affix "><li class="part-title">Documentation</li><li class="chapter-item expanded "><a href="documentation/dev_docs.html"><strong aria-hidden="true">11.</strong> Developer Documentation</a></li><li class="chapter-item expanded "><a href="documentation/links.html"><strong aria-hidden="true">12.</strong> Useful Links</a></li><li class="chapter-item expanded affix "><li class="spacer"></li><li class="chapter-item expanded affix "><a href="footer/interested-students.html">For Interested Students</a></li><li class="chapter-item expanded affix "><a href="footer/contributors.html">Contributors</a></li></ol>';
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
