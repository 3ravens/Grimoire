// Copyright (C) 2026 Wim Palland
//
// Curated sentence pools for the debug test-data generator.
// Each domain contains short paragraphs (2–4 sentences each) that can be
// assembled into plausible, varied notes.  Content is deliberately realistic
// so that semantic search and RAG benchmarks produce meaningful results.
//
// This file is compiled into debug builds only and is never shipped in
// production binaries.

/// A pool of short paragraphs belonging to a single knowledge domain.
pub(crate) struct DomainPool {
    /// Display name for folder / note-title generation.
    pub name: &'static str,
    /// Tags commonly associated with this domain.
    pub tags: &'static [&'static str],
    /// Paragraphs — each is 1–4 sentences of realistic, continuous text.
    pub paragraphs: &'static [&'static str],
    /// Short title fragments for procedural title generation.
    pub title_fragments: &'static [&'static str],
}

/// All domain pools used by the generator.
pub(crate) fn domain_pools() -> Vec<DomainPool> {
    vec![
        // ── Technology ──────────────────────────────────────────────────
        DomainPool {
            name: "Technology",
            tags: &["tech", "software", "programming", "rust"],
            title_fragments: &[
                "Understanding", "A guide to", "Notes on", "How", "The basics of",
                "Deep dive into", "Practical", "Modern",
            ],
            paragraphs: &[
                "Rust's ownership model eliminates entire classes of bugs at compile time. \
                 The borrow checker ensures that references are always valid and that data races \
                 cannot occur. This makes Rust particularly well-suited for systems programming \
                 where safety and performance are both critical.",

                "WebAssembly enables running compiled code in the browser at near-native speed. \
                 Languages like Rust, C++, and Go can target Wasm, opening up browser-based \
                 applications that were previously impossible. The Wasm sandbox also provides \
                 strong security guarantees by isolating the running code from the host system.",

                "SQLite is an embedded relational database that requires no server process. \
                 It stores the entire database in a single cross-platform file and supports \
                 ACID transactions. SQLite is the most widely deployed database engine in the \
                 world, powering everything from mobile apps to web browsers.",

                "Containerisation with Docker packages an application and its dependencies \
                 into a lightweight, portable image. Containers share the host kernel but run \
                 in isolated user-space environments. This makes them much more efficient than \
                 full virtual machines while still providing strong isolation boundaries.",

                "Continuous integration automates building and testing code every time a change \
                 is pushed to the repository. Combined with continuous deployment, it can push \
                 passing changes directly to production. This shortens the feedback loop and \
                 reduces the risk of large, difficult-to-debug merges.",

                "The Linux kernel uses a monolithic architecture where all core services run \
                 in kernel space. Device drivers can be compiled directly into the kernel or \
                 loaded as modules at runtime. This design choice prioritises performance over \
                 the microkernel goal of minimising trusted code.",

                "REST APIs use standard HTTP methods — GET, POST, PUT, DELETE — to expose \
                 resources identified by URLs. Each request is stateless, meaning the server \
                 does not store client context between calls. Authentication is typically \
                 handled via tokens passed in the Authorization header.",

                "GraphQL is a query language for APIs that lets clients request exactly the \
                 data they need. Unlike REST, where the server defines fixed endpoints, GraphQL \
                 exposes a single endpoint and lets the client specify the shape of the response. \
                 This reduces over-fetching and under-fetching of data at the cost of more \
                 complex server-side resolution logic.",

                "The Actor model treats each actor as an isolated unit of computation that \
                 communicates only through message passing. Erlang and Akka are well-known \
                 implementations. The model naturally handles distribution and fault tolerance \
                 because actors can be moved across machines without changing their semantics.",

                "TypeScript adds static type checking to JavaScript, catching errors before \
                 code reaches production. The type system is structural rather than nominal, \
                 meaning types are compatible if they have the same shape regardless of their \
                 declared names. This maps well to the dynamic nature of JavaScript objects.",

                "PostgreSQL supports advanced indexing strategies including B-tree, GiST, GIN, \
                 and BRIN. Each index type is optimised for different query patterns — GIN for \
                 full-text search, GiST for geometric data, and BRIN for very large tables with \
                 natural sort order. Choosing the right index can reduce query time from minutes \
                 to milliseconds.",

                "Kubernetes orchestrates containerised workloads across a cluster of machines. \
                 It handles scheduling, service discovery, load balancing, and self-healing. \
                 The desired state is declared in YAML manifests, and Kubernetes continuously \
                 works to reconcile the actual state with the declared state.",

                "The Git version control system stores snapshots of the entire project rather \
                 than file-level diffs. Branches are lightweight pointers to commits, making \
                 branching and merging fast operations. The distributed nature of Git means \
                 every clone is a full backup of the repository history.",

                "Zero-knowledge proofs allow one party to prove to another that a statement is \
                 true without revealing any information beyond the validity of the statement \
                 itself. They have applications in authentication, blockchain privacy, and \
                 verifiable computation. The mathematics behind them relies on polynomial \
                 commitments and interactive oracle proofs.",

                "The CAP theorem states that a distributed data store can provide at most two \
                 of three guarantees: Consistency, Availability, and Partition tolerance. In \
                 practice, network partitions are inevitable, so systems must choose between \
                 being strongly consistent (CP) or always available (AP) during a partition.",
            ],
        },

        // ── Science ─────────────────────────────────────────────────────
        DomainPool {
            name: "Science",
            tags: &["science", "biology", "physics", "research"],
            title_fragments: &[
                "The science of", "How", "Exploring", "What we know about",
                "A primer on", "Understanding", "The physics of", "Why",
            ],
            paragraphs: &[
                "Mitochondria are often called the powerhouses of the cell because they \
                 produce ATP through oxidative phosphorylation. They have their own DNA, \
                 inherited exclusively from the mother, which supports the endosymbiotic \
                 theory — the idea that mitochondria were once free-living bacteria that \
                 entered into a symbiotic relationship with early eukaryotic cells.",

                "CRISPR-Cas9 is a gene-editing technology adapted from a natural defence \
                 mechanism in bacteria. It uses a guide RNA to target a specific DNA sequence, \
                 and the Cas9 enzyme cuts the DNA at that location. The cell's repair machinery \
                 can then be harnessed to disable a gene or insert a new sequence. The \
                 technology has enormous potential for treating genetic diseases.",

                "The Standard Model of particle physics describes three of the four fundamental \
                 forces — electromagnetic, strong nuclear, and weak nuclear — and classifies all \
                 known elementary particles. It does not include gravity, which is described \
                 separately by general relativity. The Higgs boson, confirmed in 2012, was the \
                 last predicted particle to be observed.",

                "Quantum entanglement occurs when particles become correlated in such a way \
                 that measuring one instantly determines the state of the other, regardless of \
                 distance. Einstein called it 'spooky action at a distance'. Entanglement is a \
                 key resource for quantum computing and quantum cryptography.",

                "Plate tectonics explains how the Earth's lithosphere is broken into plates \
                 that move over the asthenosphere. At divergent boundaries, plates move apart \
                 and new crust is formed. At convergent boundaries, one plate subducts beneath \
                 another, driving volcanism and mountain building. The theory unified geology \
                 in the 1960s and explains earthquakes, volcanoes, and continental drift.",

                "The human genome contains roughly 3 billion base pairs organised into 23 \
                 chromosome pairs. Only about 1.5% of the genome codes for proteins. The \
                 remaining non-coding DNA includes regulatory elements, structural components, \
                 and remnants of ancient viral insertions. Understanding non-coding regions is \
                 one of the frontiers of genomics.",

                "Photosynthesis converts light energy into chemical energy stored in glucose. \
                 The light-dependent reactions split water molecules, releasing oxygen as a \
                 by-product. The light-independent Calvin cycle uses the resulting ATP and \
                 NADPH to fix carbon dioxide into organic compounds. This process sustains \
                 virtually all life on Earth directly or indirectly.",

                "The second law of thermodynamics states that the total entropy of an isolated \
                 system always increases over time. Entropy is a measure of disorder or the \
                 number of possible microscopic arrangements. The arrow of time — why time \
                 flows in one direction — is deeply connected to the increase of entropy.",

                "Black holes are regions of spacetime where gravity is so strong that nothing, \
                 not even light, can escape. They form when massive stars collapse at the end \
                 of their life cycle. The event horizon marks the boundary beyond which escape \
                 is impossible. Hawking radiation, a quantum effect near the event horizon, \
                 causes black holes to slowly evaporate over cosmological timescales.",

                "The germ theory of disease states that many diseases are caused by \
                 microorganisms too small to see without magnification. It replaced earlier \
                 theories based on miasma (bad air) and humoral imbalance. Hand washing in \
                 medical settings, championed by Semmelweis in the 1840s, dramatically reduced \
                 mortality rates before the mechanism was fully understood.",

                "Epigenetics studies heritable changes in gene expression that do not involve \
                 changes to the underlying DNA sequence. DNA methylation and histone modification \
                 are two key epigenetic mechanisms. Environmental factors — diet, stress, toxins \
                 — can alter epigenetic marks, and some of these changes can be passed to \
                 offspring. This challenges the strict nature-vs-nurture dichotomy.",

                "Nuclear fusion powers the stars by fusing light nuclei into heavier ones, \
                 releasing enormous energy. On Earth, achieving controlled fusion requires \
                 confining a plasma at temperatures over 100 million degrees Celsius. Magnetic \
                 confinement (tokamaks) and inertial confinement (lasers) are the two main \
                 approaches. Practical fusion power has been '30 years away' for decades, but \
                 recent advances have brought it closer than ever.",

                "The scientific method is a systematic process of observation, hypothesis \
                 formation, experimentation, and revision. A hypothesis must be falsifiable — \
                 there must be a conceivable observation that would prove it wrong. Peer review \
                 and replication are essential checks that filter out errors and fraud before \
                 results become accepted as scientific knowledge.",
            ],
        },

        // ── Philosophy ──────────────────────────────────────────────────
        DomainPool {
            name: "Philosophy",
            tags: &["philosophy", "ethics", "logic", "mind"],
            title_fragments: &[
                "On", "The nature of", "Thoughts on", "Why", "Exploring",
                "A defence of", "The problem of", "Reflections on",
            ],
            paragraphs: &[
                "Stoicism teaches that we should focus only on what is within our control — \
                 our judgments, choices, and actions — and accept everything else with \
                 equanimity. Epictetus opened his Enchiridion with this distinction, and it \
                 remains the foundation of the entire Stoic system. The practice of negative \
                 visualisation helps cultivate gratitude by imagining the loss of what we \
                 currently take for granted.",

                "The trolley problem is a thought experiment in ethics: a runaway trolley is \
                 headed toward five people tied to the track, and you can pull a lever to \
                 divert it to a side track with one person. Utilitarianism says pull the lever \
                 — save five at the cost of one. Deontological ethics says do not — using a \
                 person as a means violates their dignity. The problem exposes deep tensions \
                 between consequentialist and rule-based moral frameworks.",

                "Descartes' famous 'I think, therefore I am' (cogito ergo sum) was the \
                 foundation he arrived at after methodically doubting everything he could — \
                 sensory experience, memory, even mathematical truths. The one thing he could \
                 not doubt was that he was doubting. From this single certainty he attempted \
                 to rebuild all knowledge on a rational foundation.",

                "Existentialism holds that existence precedes essence — we are born without a \
                 predetermined purpose and must create our own meaning through choices and \
                 actions. Sartre argued that this radical freedom is both exhilarating and \
                 terrifying, and that we often flee from it into 'bad faith', pretending we \
                 have no choice. Camus compared the human search for meaning to Sisyphus \
                 endlessly pushing his boulder — and concluded we must imagine Sisyphus happy.",

                "Utilitarianism evaluates actions based on their consequences, specifically \
                 the net happiness or pleasure they produce. Bentham proposed a hedonic \
                 calculus to quantify this. Mill refined the theory by distinguishing higher \
                 and lower pleasures — 'it is better to be a human dissatisfied than a pig \
                 satisfied'. Critics argue that utilitarianism can justify deeply unjust \
                 actions if they produce enough aggregate happiness.",

                "The Chinese Room argument, proposed by Searle, challenges the idea that \
                 running a program can produce genuine understanding. Imagine a person in a \
                 room following English instructions to manipulate Chinese symbols — from the \
                 outside, the responses appear intelligent, but the person understands no \
                 Chinese. Searle argues that computers are in the same position: they \
                 manipulate symbols syntactically without semantic understanding.",

                "Free will remains one of the most contested concepts in philosophy. \
                 Determinists argue that all events, including human decisions, are the \
                 necessary result of prior causes. Compatibilists redefine free will as the \
                 ability to act according to one's desires without external coercion — which \
                 is compatible with determinism. Libertarians about free will argue that \
                 genuine indeterminacy at the quantum level or in agent causation makes room \
                 for truly free choices.",

                "The Ship of Theseus paradox asks: if every plank of a ship is gradually \
                 replaced, is it still the same ship? The problem challenges our intuitions \
                 about identity over time and applies to everything from personal identity \
                 (our cells are constantly replaced) to software (a program refactored line \
                 by line). Some philosophers resolve it by distinguishing numerical identity \
                 from qualitative identity.",

                "Virtue ethics, rooted in Aristotle, focuses on character rather than rules \
                 or consequences. A virtuous act is what a virtuous person would do in the \
                 circumstances. The goal is eudaimonia — human flourishing — achieved through \
                 the cultivation of virtues like courage, wisdom, temperance, and justice. \
                 Unlike deontology or utilitarianism, virtue ethics provides no decision \
                 procedure; it requires practical wisdom (phronesis) developed through \
                 experience.",

                "The hard problem of consciousness, coined by David Chalmers, asks why and \
                 how physical processes in the brain give rise to subjective experience — \
                 the 'what it is like' to be something. Explaining the neural correlates of \
                 consciousness (the easy problems) does not explain why there is experience \
                 at all rather than just information processing in the dark. Panpsychism, the \
                 view that consciousness is a fundamental property of matter, is one radical \
                 response to the hard problem.",

                "The is-ought gap, identified by Hume, points out that you cannot logically \
                 derive a statement about what ought to be from statements solely about what \
                 is. Any argument that moves from descriptive premises to a normative \
                 conclusion contains a hidden value judgment. This insight challenges moral \
                 realism and grounds many forms of moral anti-realism and non-cognitivism.",
            ],
        },

        // ── History ─────────────────────────────────────────────────────
        DomainPool {
            name: "History",
            tags: &["history", "ancient", "modern", "civilisation"],
            title_fragments: &[
                "The rise of", "The fall of", "A history of", "How",
                "The", "Understanding", "Origins of", "The story of",
            ],
            paragraphs: &[
                "The Roman Republic collapsed over several generations as military commanders \
                 like Marius, Sulla, Pompey, and Caesar accumulated personal power at the \
                 expense of the Senate's authority. The final transition to empire was \
                 gradual — Octavian (Augustus) maintained the forms of the Republic while \
                 concentrating power in his own hands. The Roman Empire that emerged would \
                 endure for another five centuries in the West and over a millennium in \
                 the East.",

                "The printing press, invented by Gutenberg around 1440, is widely regarded \
                 as one of the most transformative technologies in human history. It \
                 dramatically reduced the cost of producing books, breaking the Church's \
                 monopoly on written knowledge. The resulting spread of literacy and ideas \
                 fuelled the Reformation, the Scientific Revolution, and the Enlightenment.",

                "The Silk Road was not a single road but a network of trade routes connecting \
                 China to the Mediterranean. Goods, ideas, religions, and diseases all \
                 travelled along these routes for over 1,500 years. Buddhism spread from \
                 India to China along the Silk Road; the Black Death likely followed the same \
                 paths in the 14th century. The Silk Road declined only when maritime trade \
                 became faster and safer.",

                "The Industrial Revolution began in Britain in the late 18th century and \
                 transformed society from agrarian to industrial in a few generations. Steam \
                 power, mechanised textile production, and iron production were the key \
                 technologies. The social consequences were enormous: urbanisation, the rise \
                 of the factory system, new class structures, and ultimately the political \
                 movements — socialism, trade unionism, suffrage — that shaped the modern world.",

                "The French Revolution of 1789 overthrew the Bourbon monarchy and established \
                 a republic founded on principles of liberty, equality, and fraternity. The \
                 Revolution abolished feudalism, nationalised Church property, and introduced \
                 the metric system. But it also descended into the Terror, where tens of \
                 thousands were executed by guillotine. Napoleon's rise from the chaos \
                 illustrates how revolutions are often followed by strong-man rule.",

                "The Mongol Empire, founded by Genghis Khan in 1206, became the largest \
                 contiguous land empire in history. Mongol armies were fast, disciplined, \
                 and psychologically terrifying — cities that resisted were annihilated. Yet \
                 the Pax Mongolica that followed created a period of peace and stability across \
                 Eurasia that enabled trade and cultural exchange on an unprecedented scale.",

                "World War I began in 1914 after the assassination of Archduke Franz Ferdinand, \
                 but its deeper causes included complex alliance systems, imperial competition, \
                 nationalism, and militarism. The war introduced industrial-scale killing: \
                 machine guns, poison gas, and artillery fire turned battlefields into \
                 slaughterhouses. The Treaty of Versailles imposed punitive terms on Germany, \
                 creating grievances that directly fed into the rise of Nazism.",

                "The Renaissance, beginning in 14th-century Italy, marked a revival of \
                 classical learning and a shift toward humanism — the idea that human beings \
                 and their achievements matter. Artists like Leonardo and Michelangelo pushed \
                 naturalistic representation to new heights. The invention of linear perspective \
                 in painting was as much a mathematical breakthrough as an artistic one.",

                "The Cold War was a geopolitical struggle between the United States and the \
                 Soviet Union that lasted from roughly 1947 to 1991. It was fought through \
                 proxy wars (Korea, Vietnam, Afghanistan), nuclear arms racing, space \
                 competition, and ideological propaganda. The threat of mutual assured \
                 destruction prevented direct military conflict between the superpowers. \
                 The Soviet Union's collapse in 1991 left the United States as the sole \
                 superpower.",

                "The transatlantic slave trade forcibly transported roughly 12 million \
                 Africans to the Americas between the 16th and 19th centuries. The Middle \
                 Passage — the voyage across the Atlantic — had mortality rates of 10–20%. \
                 The economic impact on Africa was devastating, while the labour of enslaved \
                 people built much of the wealth of the colonial powers. Abolition took \
                 decades of political struggle, economic pressure, and resistance by enslaved \
                 people themselves.",

                "The Meiji Restoration of 1868 transformed Japan from a feudal, isolated \
                 country into a modern industrial power within a single generation. The new \
                 government sent missions to study Western institutions, built railways and \
                 telegraphs, established universal education, and created a modern military. \
                 By 1905, Japan had defeated Russia — the first time an Asian power had \
                 defeated a European power in modern warfare.",
            ],
        },

        // ── Cooking & Food ──────────────────────────────────────────────
        DomainPool {
            name: "Cooking",
            tags: &["cooking", "food", "recipes", "fermentation"],
            title_fragments: &[
                "How to make", "The art of", "Understanding", "A guide to",
                "Perfecting", "The science of", "Traditional", "Homemade",
            ],
            paragraphs: &[
                "Fermentation transforms food through the action of beneficial microorganisms. \
                 Lactic acid bacteria convert sugars into lactic acid, creating the tangy \
                 flavour of yogurt, sauerkraut, and kimchi. Alcoholic fermentation by yeast \
                 produces ethanol and carbon dioxide, the basis of beer, wine, and bread. \
                 Fermented foods are rich in probiotics that support gut microbiome diversity.",

                "The Maillard reaction is a chemical process between amino acids and reducing \
                 sugars that occurs when food is heated above roughly 140°C. It creates the \
                 complex, savoury flavours and brown colour of seared meat, toasted bread, \
                 roasted coffee, and baked goods. The reaction is distinct from caramelisation, \
                 which involves only sugar and occurs at higher temperatures.",

                "Sourdough bread uses a symbiotic culture of wild yeast and lactic acid \
                 bacteria instead of commercial baker's yeast. The bacteria produce lactic \
                 and acetic acids that give sourdough its characteristic tang and improve \
                 keeping quality. Maintaining a sourdough starter — feeding it flour and \
                 water regularly — is a commitment that rewards bakers with complex flavour \
                 and a more digestible loaf.",

                "The five mother sauces of French cuisine — béchamel, velouté, espagnole, \
                 sauce tomate, and hollandaise — were codified by Auguste Escoffier in the \
                 19th century. Every classical French sauce is a variation or derivative of \
                 one of these five. A roux (flour cooked in fat) thickens the first three; \
                 hollandaise is an emulsion of egg yolks and butter.",

                "Knife skills are the foundation of efficient cooking. The claw grip — \
                 curling your fingertips under and guiding the blade against your knuckles — \
                 protects your fingers while allowing precise cuts. A sharp knife is safer \
                 than a dull one because it requires less force and is less likely to slip. \
                 Different cuts — dice, julienne, brunoise, chiffonade — produce uniform \
                 pieces that cook evenly.",

                "Umami, the fifth basic taste alongside sweet, sour, salty, and bitter, was \
                 identified by Japanese chemist Kikunae Ikeda in 1908. It is the savoury \
                 taste of glutamate and certain nucleotides found in foods like soy sauce, \
                 Parmesan cheese, tomatoes, mushrooms, and cured meats. Umami enhances and \
                 rounds out other flavours, which is why ingredients rich in it — anchovies, \
                 fish sauce, miso — are used to add depth to dishes.",

                "Tempering chocolate involves heating and cooling it to specific temperatures \
                 to stabilise the cocoa butter crystals. Properly tempered chocolate is glossy, \
                 snaps cleanly, and does not bloom (develop white streaks) at room temperature. \
                 The process requires precise temperature control — dark chocolate is tempered \
                 at about 31–32°C after being heated to 45°C and cooled to 27°C.",

                "Pasta fresca — fresh pasta made with eggs and soft wheat flour — has a tender, \
                 silky texture that dried pasta cannot replicate. The classic ratio is one large \
                 egg per 100 grams of flour. Kneading develops the gluten network that gives \
                 pasta its structure and bite. Fresh pasta cooks in 2–3 minutes, compared to \
                 8–12 minutes for dried pasta.",

                "Regional cuisines evolve from the intersection of available ingredients, \
                 climate, trade routes, and cultural values. Mediterranean cooking is built \
                 around olive oil, fresh vegetables, seafood, and herbs because those are what \
                 the region produces. The principles of a cuisine — the techniques, flavour \
                 combinations, and meal structures — are often more important than specific \
                 recipes. Understanding principles lets you improvise within the tradition.",

                "The wok is one of the most versatile cooking vessels ever invented. Its \
                 shape concentrates intense heat at the bottom while the sloping sides provide \
                 cooler zones for resting food. Stir-frying in a wok requires high heat, \
                 constant movement, and ingredients prepared in advance (mise en place). The \
                 wok hei — 'breath of the wok' — refers to the subtle smoky flavour imparted \
                 by the intense heat and rapid cooking of a well-seasoned carbon steel wok.",
            ],
        },

        // ── Fitness & Health ────────────────────────────────────────────
        DomainPool {
            name: "Fitness",
            tags: &["fitness", "health", "exercise", "nutrition"],
            title_fragments: &[
                "The benefits of", "How to", "A guide to", "Understanding",
                "Building", "The science of", "Improving", "Starting",
            ],
            paragraphs: &[
                "Progressive overload is the principle that to continue making gains in \
                 strength or endurance, you must gradually increase the stress placed on \
                 the body. This can mean adding weight, increasing repetitions, reducing \
                 rest time, or improving form. Without progressive overload, the body \
                 adapts to the current stimulus and progress stalls. Tracking workouts is \
                 essential to ensure overload is actually happening over time.",

                "Sleep is arguably the most underrated component of physical recovery. \
                 During deep sleep, the body releases growth hormone, repairs muscle tissue, \
                 and consolidates motor learning from the day's training. Chronic sleep \
                 deprivation impairs reaction time, decision-making, and glucose metabolism. \
                 Most adults need 7–9 hours, and athletes in heavy training often need more.",

                "Zone 2 cardio — steady-state exercise at 60–70% of maximum heart rate — \
                 builds the aerobic base by improving mitochondrial density and fat oxidation. \
                 It should feel conversational: you can talk but not sing. Most endurance \
                 athletes spend 80% of their training time in Zone 2, reserving high-intensity \
                 work for the remaining 20%. This polarised approach balances stimulus with \
                 recovery.",

                "Protein timing matters less than total daily intake for most people, but \
                 distributing protein across 3–4 meals spaced 3–5 hours apart maximises \
                 muscle protein synthesis. A target of 1.6–2.2 grams of protein per kilogram \
                 of body weight per day is sufficient for most athletes. Whole food sources \
                 are generally preferable to supplements, though whey protein is convenient \
                 and highly bioavailable.",

                "Compound exercises — squats, deadlifts, bench press, pull-ups, rows — work \
                 multiple muscle groups and joints simultaneously. They are more time-efficient \
                 than isolation exercises and produce a greater hormonal response. A training \
                 program built around compound lifts, with isolation work added for specific \
                 weaknesses, is the standard recommendation for both strength and hypertrophy.",

                "The mind-muscle connection refers to consciously focusing on the target \
                 muscle during an exercise. Research suggests that it can increase muscle \
                 activation, particularly in isolation exercises. It is less effective for \
                 compound lifts where attention should be on movement quality. Beginners \
                 benefit most from simply learning proper technique before worrying about \
                 mental focus.",

                "VO2 max is the maximum rate of oxygen consumption measured during incremental \
                 exercise. It is a strong predictor of endurance performance and overall \
                 cardiovascular health. While genetics set an upper ceiling, training can \
                 improve VO2 max by 15–25% in most people. High-intensity interval training \
                 is particularly effective at raising VO2 max in a time-efficient manner.",

                "Hydration status affects cognitive function, thermoregulation, and exercise \
                 performance. A loss of just 2% of body weight through sweat can measurably \
                 impair both strength and endurance. Electrolytes — sodium, potassium, \
                 magnesium — are lost alongside water and must be replaced during prolonged \
                 exercise. Urine colour is a practical proxy for hydration status: pale \
                 straw is the target.",

                "Mobility and flexibility are distinct but related qualities. Flexibility \
                 is the passive range of motion of a joint; mobility is the active control \
                 through that range. Both matter for injury prevention and movement quality. \
                 Static stretching before exercise can temporarily reduce power output; \
                 dynamic warm-ups are preferred for pre-workout preparation, with static \
                 stretching reserved for post-workout cool-down.",

                "Stress is catabolic — chronic elevation of cortisol breaks down muscle \
                 tissue, promotes fat storage around the abdomen, and impairs recovery. \
                 Training is itself a stressor; the art of programming is balancing training \
                 stress with adequate recovery. Sleep, nutrition, and stress management are \
                 not separate from training — they are an integral part of it.",

                "The concept of minimum effective dose applies to exercise as much as to \
                 medicine. Beyond a certain volume, additional sets produce diminishing \
                 returns and increase injury risk. For beginners, 3–6 working sets per \
                 muscle group per week can produce significant gains. Advanced lifters \
                 may need 10–20 sets, but marginal gains shrink as training age increases.",
            ],
        },

        // ── Writing & Creativity ────────────────────────────────────────
        DomainPool {
            name: "Writing",
            tags: &["writing", "creativity", "journaling", "pkm"],
            title_fragments: &[
                "How to", "The craft of", "On", "A writer's guide to",
                "The art of", "Improving your", "Notes on", "Why",
            ],
            paragraphs: &[
                "Writing is thinking. The act of putting thoughts into words forces clarity — \
                 vague ideas that feel profound in your head often collapse under the weight \
                 of a sentence. Paul Graham observed that writing a first draft is like \
                 discovering what you think, not just recording it. This is why writing is \
                 not merely communication; it is a tool for reasoning.",

                "Show, don't tell is the most repeated writing advice for good reason. \
                 'She was angry' tells the reader what to think. 'Her knuckles went white \
                 around the mug, and she set it down without a word' shows the reader and \
                 lets them draw their own conclusion. Concrete sensory details engage the \
                 reader's imagination far more effectively than abstract labels.",

                "The Zettelkasten method treats each note as an atomic idea linked to \
                 related notes. Over time, the network of links surfaces connections that \
                 would never emerge from a hierarchical folder structure. Niklas Luhmann \
                 used this method to write over 70 books. The key insight is that meaning \
                 emerges from the structure of links, not from the categories you impose \
                 on the notes.",

                "Writer's block is often not a lack of ideas but a mismatch between your \
                 standards and your output. The solution — proposed by Anne Lamott, among \
                 others — is to write a 'shitty first draft' deliberately. Give yourself \
                 permission to write badly. You cannot edit a blank page. Once words exist, \
                 you can improve them. The first draft is about discovery; revision is about \
                 craft.",

                "Morning pages, popularised by Julia Cameron in The Artist's Way, are three \
                 pages of longhand, stream-of-consciousness writing done first thing in the \
                 morning. They are not intended to be read by anyone, including yourself in \
                 the short term. The practice clears mental clutter, surfaces submerged \
                 thoughts, and establishes a daily writing habit that bypasses the inner critic.",

                "Kill your darlings is advice attributed to Faulkner and others: be willing \
                 to delete sentences, paragraphs, or even chapters that you love but that do \
                 not serve the work. The attachment is to the piece's overall effectiveness, \
                 not to any particular brilliant turn of phrase. If it does not move the story \
                 forward or deepen understanding, it is a candidate for cutting.",

                "Reading widely is the most reliable way to improve as a writer. You absorb \
                 rhythm, vocabulary, and technique by osmosis through exposure to good writing. \
                 Read outside your genre, read poetry for precision of language, read non-fiction \
                 for clarity of argument. Every book is a masterclass in what works and what \
                 does not, if you pay attention.",

                "Outlining versus discovery writing (pantsing) is a false dichotomy. Most \
                 writers use a hybrid approach: a loose outline that provides direction but \
                 leaves room for discovery. The outline is a map, not a contract. When the \
                 terrain turns out to be different from what the map predicted, follow the \
                 terrain. Revise the outline later if needed.",

                "Revision is where writing actually happens. The first draft makes it exist; \
                 revision makes it good. Read your work aloud — awkward phrasing that your \
                 eyes glide over will catch in your ear. Print it out and read it on paper. \
                 Change the font. Each technique reveals different problems. Revision is not \
                 fixing typos; it is rethinking structure, argument, and voice.",

                "Constraints fuel creativity. A sonnet's fourteen lines, a haiku's syllable \
                 count, a deadline — limitations force you to make decisions and commit. \
                 Unlimited freedom is paralysing because every option remains open. Deliberate \
                 constraints (form, length, vocabulary, perspective) narrow the problem space \
                 and make it tractable. Creativity thrives within boundaries.",

                "Keeping a commonplace book — a personal collection of quotes, observations, \
                 and ideas from your reading — has been practiced by writers and thinkers for \
                 centuries. It is distinct from a journal or diary because the content comes \
                 from external sources, filtered through your judgment. Reviewing a commonplace \
                 book surfaces connections between ideas you had not previously linked.",
            ],
        },

        // ── Productivity ────────────────────────────────────────────────
        DomainPool {
            name: "Productivity",
            tags: &["productivity", "habits", "planning", "focus"],
            title_fragments: &[
                "How to", "The", "A system for", "Mastering", "Why",
                "Building", "Improving your", "The art of",
            ],
            paragraphs: &[
                "Deep work, as defined by Cal Newport, is the ability to focus without \
                 distraction on a cognitively demanding task. It is a skill that produces \
                 high-value output and is becoming increasingly rare in a world of constant \
                 notifications and open-plan offices. Cultivating deep work requires creating \
                 rituals, embracing boredom, and actively reducing shallow obligations.",

                "The Pomodoro Technique breaks work into 25-minute focused intervals separated \
                 by 5-minute breaks, with a longer break every four cycles. The timer creates \
                 a sense of urgency that reduces the urge to procrastinate. The technique works \
                 because it makes starting easy (just 25 minutes) and provides a rhythm that \
                 sustains focus throughout the day.",

                "Eisenhower's decision matrix separates tasks by urgency and importance. \
                 Important-but-not-urgent tasks — strategic planning, skill development, \
                 relationship building — are where long-term value lies but are the easiest \
                 to neglect. The matrix is not just a prioritisation tool; it is a diagnostic \
                 for whether your day is being driven by your priorities or by other people's \
                 emergencies.",

                "The two-minute rule, from David Allen's GTD system, states that if a task \
                 takes less than two minutes, you should do it immediately rather than \
                 deferring it. The cumulative overhead of tracking, reviewing, and \
                 re-deciding on small tasks often exceeds the time it takes to actually do \
                 them. The rule also provides a quick win that builds momentum.",

                "Parkinson's Law states that work expands to fill the time available for its \
                 completion. Setting artificial deadlines shorter than the natural timeframe \
                 can compress the work and reduce unproductive perfectionism. But deadlines \
                 that are too aggressive cause stress and poor quality. The art is finding \
                 the sweet spot: challenging but achievable.",

                "Habit stacking, proposed by James Clear in Atomic Habits, links a new habit \
                 to an existing one: 'After I pour my morning coffee, I will meditate for \
                 five minutes.' The existing habit becomes the trigger for the new one. This \
                 leverages the brain's existing wiring rather than trying to build a new \
                 routine from scratch. The formula is: After [current habit], I will [new habit].",

                "The Zeigarnik Effect is the tendency to remember interrupted or incomplete \
                 tasks better than completed ones. This is why unresolved problems loop in \
                 your mind while finished projects fade from memory. Writing down tasks \
                 (externalising them) reduces the cognitive load of keeping them in working \
                 memory. A trusted external system — notebook, app, list — frees mental \
                 bandwidth for actual thinking.",

                "Rituals and routines are not the same thing. A routine is a sequence of \
                 actions performed automatically. A ritual adds meaning and intention — it \
                 signals to your brain that a transition is occurring. Lighting a candle \
                 before writing, putting on headphones before coding, closing all tabs at \
                 the end of the day — small rituals mark boundaries and make states easier \
                 to enter and exit.",

                "The planning fallacy is the systematic tendency to underestimate how long \
                 a task will take, even when you have experience with similar tasks. \
                 Reference class forecasting — basing estimates on how long similar projects \
                 actually took rather than on your optimistic breakdown — is one of the few \
                 techniques shown to improve accuracy. Adding a buffer of 25–50% accounts \
                 for unknown unknowns.",

                "Context switching — jumping between tasks — has a cognitive cost that is \
                 not immediately visible. Each switch leaves an 'attention residue' on the \
                 previous task, reducing performance on the new one. Batching similar tasks \
                 together and protecting blocks of uninterrupted time produces more output \
                 per hour than multitasking, even if it feels less productive in the moment.",

                "Saying no is a productivity multiplier. Every yes to a request is a no to \
                 something else — often something more important. The most productive people \
                 guard their attention as jealously as their time. A polite, timely no is \
                 far better than a reluctant yes followed by under-delivery or quiet \
                 resentment. Practice phrases: 'I'm afraid I can't take that on right now' \
                 or 'That doesn't fit my current priorities.'",
            ],
        },

        // ── Gardening & Nature ──────────────────────────────────────────
        DomainPool {
            name: "Gardening",
            tags: &["gardening", "nature", "plants", "ecology"],
            title_fragments: &[
                "How to grow", "The", "A guide to", "Understanding",
                "Caring for", "The ecology of", "Starting a", "Why",
            ],
            paragraphs: &[
                "Composting transforms kitchen scraps and garden waste into nutrient-rich \
                 humus through aerobic decomposition. A good compost pile balances carbon-rich \
                 browns (dried leaves, cardboard, straw) with nitrogen-rich greens (grass \
                 clippings, vegetable peelings, coffee grounds) at roughly a 30:1 ratio. \
                 Turning the pile introduces oxygen and accelerates the process. Finished \
                 compost improves soil structure, water retention, and microbial diversity.",

                "Companion planting pairs plants that benefit each other when grown together. \
                 The classic Three Sisters — corn, beans, and squash — work because the corn \
                 provides a trellis for the beans, the beans fix nitrogen in the soil, and the \
                 squash shades the ground, suppressing weeds and retaining moisture. Other \
                 effective pairs: basil with tomatoes (repels pests), marigolds with vegetables \
                 (deters nematodes), and dill with brassicas (attracts beneficial wasps).",

                "Soil pH controls the availability of nutrients to plants. Most vegetables \
                 prefer slightly acidic to neutral soil (pH 6.0–7.0). Below 5.5, nutrients \
                 like phosphorus become locked up and unavailable. Lime raises pH; sulphur \
                 lowers it. A soil test is the only reliable way to know your pH — guessing \
                 leads to over-correction and nutrient imbalances.",

                "Perennial vegetables return year after year without replanting. Asparagus \
                 beds can produce for 20 years. Rhubarb, artichokes, Jerusalem artichokes, \
                 and sorrel are cold-hardy perennials in temperate climates. Perennials build \
                 deep root systems that improve soil structure and require less water and \
                 fertiliser than annual crops once established.",

                "Pollinators — bees, butterflies, hoverflies, moths — are essential for the \
                 reproduction of roughly 75% of flowering plants and 35% of global food crops. \
                 Native pollinators are often more effective than honeybees for local plants. \
                 Providing a continuous succession of blooms from early spring to late autumn, \
                 avoiding pesticides, and leaving some bare ground for ground-nesting bees are \
                 three simple interventions that increase pollinator populations.",

                "Pruning is not just cosmetic — it directs a plant's energy. Cutting back to \
                 an outward-facing bud encourages an open, airy shape that resists disease. \
                 Removing dead, damaged, and crossing branches (the three D's) is the first \
                 step in any pruning job. Timing matters: spring-flowering shrubs are pruned \
                 after they bloom; summer-flowering shrubs are pruned in late winter before \
                 new growth begins.",

                "Seed saving is the practice of collecting seeds from open-pollinated \
                 (non-hybrid) plants for replanting in future seasons. It preserves genetic \
                 diversity, adapts varieties to local conditions over generations, and reduces \
                 dependence on commercial seed suppliers. Tomatoes, beans, peas, and lettuce \
                 are among the easiest crops for beginning seed savers. Wet-processed seeds \
                 (tomatoes, cucumbers) must be fermented briefly to remove germination-inhibiting \
                 gel coatings.",

                "Mulching covers the soil surface with organic material — straw, wood chips, \
                 leaf mould, compost — to suppress weeds, retain moisture, and moderate soil \
                 temperature. A 5–10 cm layer is typically sufficient. Organic mulches break \
                 down over time, adding organic matter to the soil. Avoid piling mulch directly \
                 against plant stems and tree trunks, which can cause rot.",

                "Native plants are adapted to local climate, soil, and wildlife and generally \
                 require less water, fertiliser, and pest control than exotic species. They \
                 support local food webs — many native insects can only feed on plants with \
                 which they co-evolved. A garden that is 70% native biomass provides ecological \
                 value while still accommodating favourite non-invasive exotics.",

                "No-dig gardening avoids disturbing the soil structure by never tilling or \
                 digging. Instead, organic matter is applied to the surface as a mulch, and \
                 soil organisms — worms, fungi, bacteria — incorporate it naturally. The method \
                 preserves soil structure, protects mycorrhizal fungal networks, reduces weed \
                 germination (buried weed seeds stay buried), and sequesters carbon. Established \
                 no-dig beds require less weeding and watering than conventionally cultivated soil.",
            ],
        },

        // ── Travel ──────────────────────────────────────────────────────
        DomainPool {
            name: "Travel",
            tags: &["travel", "places", "culture", "adventure"],
            title_fragments: &[
                "A visit to", "Exploring", "The best of", "A guide to",
                "Notes from", "Travels in", "Why visit", "The beauty of",
            ],
            paragraphs: &[
                "Slow travel prioritises depth over breadth. Instead of visiting ten cities in \
                 fourteen days, spend a week in one neighbourhood — visit the same café each \
                 morning, shop at the local market, walk without a map. The goal is to \
                 experience a place rather than tick sights off a list. Slow travel produces \
                 richer memories and a more genuine understanding of a culture.",

                "Kyoto's temple gardens are designed according to principles developed over \
                 centuries. Borrowed scenery (shakkei) incorporates distant mountains or trees \
                 into the garden's composition. The raked gravel of karesansui (dry landscape) \
                 gardens represents water ripples. Moss is cultivated with painstaking care. \
                 These gardens are not meant to be seen all at once — they reveal themselves \
                 gradually as you walk the path, each view deliberately framed.",

                "The Trans-Siberian Railway stretches 9,289 km from Moscow to Vladivostok, \
                 crossing eight time zones. The full journey takes about seven days non-stop, \
                 but breaking it into segments — stopping at Lake Baikal, Irkutsk, Ulan-Ude — \
                 transforms the trip from a marathon into an exploration of Russia's vastness. \
                 The train itself is a microcosm: passengers share food, stories, and silence \
                 across language barriers.",

                "Street food is the most direct way to engage with a cuisine. In Bangkok, \
                 hawker stalls serve dishes perfected over decades by cooks who make only one \
                 or two things — a pad Thai specialist, a som tam vendor, a mango sticky rice \
                 cart. The best stalls often have one thing in common: a long queue of locals. \
                 Street food also reveals how a culture balances flavour, cost, and speed in \
                 everyday eating.",

                "Travel during shoulder season — the weeks between peak and off-peak — often \
                 provides the best experience. Prices are lower, crowds are thinner, and the \
                 weather is usually still pleasant. In Mediterranean Europe, May and September \
                 offer warm days without the July-August crush. In Southeast Asia, the weeks \
                 just after the monsoon end bring lush green landscapes and clear skies.",

                "Learning a few phrases in the local language — hello, thank you, please, \
                 excuse me, how much? — changes the dynamic of every interaction. It signals \
                 respect and effort, and often opens doors that remain closed to those who \
                 assume English will be spoken. Pronunciation matters less than the willingness \
                 to try. A smile and a badly pronounced 'thank you' in the local language \
                 goes further than fluent English delivered without warmth.",

                "The Camino de Santiago is a network of pilgrimage routes across Europe \
                 converging at the cathedral in Santiago de Compostela, Spain. The most \
                 popular route, the Camino Francés, takes about 30–35 days on foot. Pilgrims \
                 carry their own packs, sleep in communal albergues, and share meals and \
                 stories with strangers who become friends. The physical challenge is real, \
                 but the mental and social dimensions are what most pilgrims remember.",

                "Travel journals capture details that photographs miss — the smell of a \
                 market, an overheard conversation, the texture of a wall, the way light \
                 fell at a particular moment. Writing even a few sentences each evening \
                 anchors the day's experiences in memory. Re-reading a travel journal years \
                 later activates sensory recall in a way that scrolling through photos often \
                 does not. The act of describing forces you to notice more deeply.",

                "The Icelandic landscape feels like another planet. Volcanic rock covered \
                 in moss, steaming geothermal vents, glaciers that flow down to the sea, \
                 and waterfalls that appear around every bend in the road. The Ring Road \
                 circles the entire island and can be driven in about a week, but every \
                 turn tempts you to stop and stare. In summer, the sun barely sets; in \
                 winter, the Northern Lights flicker across a dark sky.",

                "Cultural humility is the recognition that you are a guest in someone else's \
                 home. Dress modestly where it is expected, ask before photographing people, \
                 and accept that some spaces — temples, ceremonies, private homes — may not \
                 be open to you. The point of travel is not to impose your expectations on a \
                 place but to let the place expand your understanding of what is normal. \
                 Some of the most memorable travel moments come from participating in \
                 rhythms and customs that are entirely unfamiliar.",
            ],
        },

        // ── Music ───────────────────────────────────────────────────────
        DomainPool {
            name: "Music",
            tags: &["music", "theory", "composition", "listening"],
            title_fragments: &[
                "The", "Understanding", "A guide to", "Why",
                "How to", "The art of", "Exploring", "On",
            ],
            paragraphs: &[
                "The circle of fifths organises all twelve pitches of the chromatic scale \
                 into a circular diagram where each step clockwise is a perfect fifth. It \
                 shows key relationships, chord progressions, and modulation paths. Closely \
                 related keys sit next to each other; distant keys sit opposite. The circle \
                 is not just a memorisation aid — it encodes centuries of harmonic practice \
                 in a single elegant shape.",

                "Counterpoint is the art of combining independent melodic lines into a \
                 coherent whole. Bach's fugues are the canonical examples: multiple voices, \
                 each with its own melodic identity, interweaving according to strict rules \
                 of voice leading and dissonance treatment. Species counterpoint, taught by \
                 Fux in 1725, remains the standard pedagogical method — students progress \
                 through five species (note-against-note, two-against-one, four-against-one, \
                 syncopation, and florid) before attempting free composition.",

                "The overtone series is the natural sequence of frequencies that resonate \
                 when a string or column of air vibrates. The first overtone is the octave, \
                 then the fifth, then the fourth, major third, minor third, and so on in \
                 increasingly close intervals. The overtone series explains why major chords \
                 sound consonant (the major triad appears in the first six overtones) and why \
                 certain intervals and chord voicings feel stable or unstable to the ear.",

                "Jazz harmony extends functional harmony with chord extensions (7ths, 9ths, \
                 11ths, 13ths), altered dominants, and tritone substitutions. The ii-V-I \
                 progression is the fundamental building block, endlessly varied and \
                 reharmonised. Modal jazz, pioneered by Miles Davis on Kind of Blue, shifted \
                 the improvisational focus from chord changes to scales (modes), giving \
                 soloists more harmonic freedom and a more spacious, meditative feel.",

                "Vinyl records store sound as physical undulations in a spiral groove. A \
                 stylus tracing the groove vibrates, and those vibrations are amplified. \
                 The format imposes constraints — bass must be summed to mono, high \
                 frequencies are pre-emphasised on cutting and de-emphasised on playback \
                 (RIAA equalisation), and the inner grooves have lower fidelity than the \
                 outer grooves because the linear velocity decreases. Despite — or because \
                 of — these constraints, many listeners prefer vinyl's sound.",

                "Minimalism in music uses repetition, gradual transformation, and limited \
                 materials. Steve Reich's 'Music for 18 Musicians' pulses with interlocking \
                 rhythmic patterns that shift slowly over an hour. Philip Glass uses additive \
                 processes where notes are added one by one to repeating figures. The effect \
                 is hypnotic — small changes become momentous when sustained attention is \
                 paid to them. Minimalism rejected the complexity and inaccessibility of \
                 mid-20th-century serialism.",

                "A DAW (Digital Audio Workstation) is a complete recording studio in software. \
                 Modern DAWs — Ableton Live, Logic Pro, Reaper, FL Studio — provide \
                 multi-track recording, MIDI sequencing, virtual instruments, effects \
                 processing, and mixing tools. The barrier to producing professional-quality \
                 music has never been lower: a laptop, an audio interface, and a decent pair \
                 of headphones or monitors is enough to create and release music globally.",

                "The blues is the foundation of most 20th-century American popular music — \
                 jazz, R&B, rock, soul, and hip-hop all trace roots through the blues. The \
                 12-bar blues form, the blues scale (minor pentatonic with flat fifth), and \
                 the call-and-response pattern are its core elements. The blues is not just \
                 a musical form but an expressive vocabulary — bending notes, rough timbres, \
                 and melodic phrasing that mirrors speech. It emerged from the experience of \
                 Black Americans in the Deep South and carries that history in its very sound.",

                "Room acoustics can make or break a listening experience. Hard parallel \
                 surfaces create standing waves and flutter echoes; absorption (curtains, \
                 carpet, acoustic panels) deadens reflections, while diffusion scatters them \
                 for a more natural sound. The ideal listening room balances absorption, \
                 diffusion, and reflection. Speaker placement is at least as important as \
                 room treatment — small changes in position and toe-in angle produce large \
                 changes in stereo imaging and bass response.",

                "Learning an instrument reshapes the brain. Studies show that musicians have \
                 larger corpus callosa (the bridge between brain hemispheres), enhanced \
                 auditory processing, and stronger executive function. Starting as a child \
                 produces the largest structural changes, but learning as an adult still \
                 yields cognitive benefits. The key variable is consistent practice over \
                 years, not early starting age. Playing music engages more brain regions \
                 simultaneously than almost any other human activity.",
            ],
        },

        // ── Finance ─────────────────────────────────────────────────────
        DomainPool {
            name: "Finance",
            tags: &["finance", "investing", "economics", "money"],
            title_fragments: &[
                "The basics of", "Understanding", "A guide to", "How to",
                "Why", "The case for", "Personal", "Building",
            ],
            paragraphs: &[
                "Compound interest is the eighth wonder of the world, according to the \
                 (possibly apocryphal) Einstein quote. A 7% annual return doubles money \
                 roughly every 10 years — the rule of 72. The key variable is time: £5,000 \
                 invested at 25 grows more than £10,000 invested at 35, assuming the same \
                 return. Delaying by a decade is far more expensive than it feels in the moment.",

                "Index funds track a market index rather than trying to beat it through stock \
                 picking. Over 20+ year windows, broad market index funds have historically \
                 outperformed the vast majority of actively managed funds after fees. The \
                 reason is simple arithmetic: the average active manager must underperform \
                 the market by the amount of their fees. Low-cost index investing is the \
                 closest thing to a free lunch in finance.",

                "An emergency fund of 3–6 months of living expenses is the foundation of any \
                 financial plan. It prevents you from having to sell investments at the worst \
                 possible time or going into high-interest debt when an unexpected expense \
                 arrives. The money should be held in a liquid, safe account — a high-yield \
                 savings account or money market fund, not the stock market. The peace of \
                 mind is worth the opportunity cost of not investing that cash.",

                "Asset allocation — the mix of stocks, bonds, real estate, and cash in a \
                 portfolio — explains the vast majority of a portfolio's return variability \
                 over time. Stock-heavy portfolios produce higher long-term returns but with \
                 gut-wrenching volatility along the way. Bond-heavy portfolios are smoother \
                 but grow more slowly. The right allocation depends on time horizon, risk \
                 tolerance, and the psychological ability to avoid panic-selling during \
                 downturns.",

                "Tax-advantaged accounts — ISAs in the UK, 401(k)s and IRAs in the US, \
                 superannuation in Australia — shield investment growth and income from tax, \
                 dramatically improving long-term returns. A pound saved in tax is a pound \
                 that compounds for decades. Filling tax-advantaged space before investing \
                 in taxable accounts is a simple, high-impact optimisation that requires no \
                 skill or luck.",

                "Inflation is a silent tax on cash. At 3% annual inflation, the purchasing \
                 power of £100 halves in roughly 24 years. Assets that produce real returns \
                 — equities, property, inflation-linked bonds — protect against this erosion. \
                 Gold has historically maintained purchasing power over very long periods but \
                 is volatile over shorter ones. Cash is for spending and emergency reserves, \
                 not for long-term wealth preservation.",

                "Dollar-cost averaging — investing a fixed amount regularly regardless of \
                 market conditions — removes the psychological burden of timing the market. \
                 It buys more shares when prices are low and fewer when prices are high, \
                 producing a lower average cost per share over time. It also builds the habit \
                 of consistent investing, which matters far more than getting the perfect \
                 entry price.",

                "The 4% rule, from the Trinity Study, suggests that a retiree can withdraw \
                 4% of their portfolio in the first year, adjusted for inflation each year \
                 thereafter, with a high probability of not running out of money over 30 \
                 years. It is a rough guideline, not a guarantee. Lower withdrawal rates \
                 (3–3.5%) are safer for longer retirements or when valuations are high at \
                 the start of retirement.",

                "Financial independence means your assets generate enough income to cover \
                 your living expenses without needing to work. The simple formula: multiply \
                 your annual expenses by 25 (the inverse of the 4% rule). If you spend \
                 £30,000 a year, you need roughly £750,000 in invested assets. The path \
                 is straightforward but not easy: earn more, spend less, invest the difference \
                 in a diversified portfolio, and wait.",

                "Behavioural finance studies how psychology affects financial decisions. \
                 Loss aversion — the pain of losing £100 is about twice as intense as the \
                 pleasure of gaining £100 — leads to panic selling in downturns and holding \
                 losing investments too long. Recency bias makes investors extrapolate recent \
                 trends into the indefinite future. Understanding these biases does not \
                 eliminate them, but it helps you recognise them and build systems \
                 (automation, rules-based plans) that protect against them.",

                "Insurance is not an investment — it is protection against catastrophic loss. \
                 Term life insurance provides pure coverage at a low cost. Whole-life and \
                 universal-life policies bundle insurance with an investment component and \
                 typically charge high fees for mediocre returns. The advice to 'buy term and \
                 invest the difference' holds for most people. Health, home, and disability \
                 insurance follow the same principle: insure what would ruin you, self-insure \
                 what would merely annoy you.",
            ],
        },

        // ── Psychology ──────────────────────────────────────────────────
        DomainPool {
            name: "Psychology",
            tags: &["psychology", "cognition", "behaviour", "mind"],
            title_fragments: &[
                "The psychology of", "Understanding", "Why we", "How",
                "The science of", "On", "Exploring", "A theory of",
            ],
            paragraphs: &[
                "Cognitive dissonance is the discomfort people feel when holding two \
                 contradictory beliefs or when their behaviour conflicts with their values. \
                 The discomfort motivates us to resolve the inconsistency — typically by \
                 changing one of the beliefs rather than the behaviour. This explains why \
                 people often double down on bad decisions: acknowledging the mistake would \
                 be more painful than maintaining the course.",

                "The Dunning-Kruger effect describes a cognitive bias where people with low \
                 competence in a domain overestimate their ability, while highly competent \
                 people underestimate theirs. Beginners lack the metacognitive skill to \
                 evaluate their own performance accurately. As competence grows, people \
                 become better calibrated — but the initial overconfidence can cause real \
                 harm in domains like medicine, investing, and driving.",

                "Attachment theory, developed by Bowlby and Ainsworth, proposes that early \
                 relationships with caregivers shape expectations about relationships \
                 throughout life. Secure attachment develops when caregivers are consistently \
                 responsive. Anxious, avoidant, and disorganised attachment styles emerge from \
                 inconsistent, neglectful, or frightening caregiving. Attachment styles are \
                 not fixed — they can shift through corrective experiences in adult \
                 relationships and in therapy.",

                "The fundamental attribution error is the tendency to attribute others' \
                 behaviour to their character while attributing our own behaviour to \
                 circumstances. When someone cuts us off in traffic, they're a reckless \
                 jerk; when we cut someone off, we had a good reason. The error is so \
                 automatic that overcoming it requires deliberate effort. Training yourself \
                 to ask 'what situation might explain this?' before judging character is a \
                 practical corrective.",

                "Confirmation bias leads people to seek, interpret, and remember information \
                 that confirms their existing beliefs while ignoring or discounting \
                 contradictory evidence. It operates unconsciously and is stronger for \
                 emotionally charged or identity-relevant beliefs. The only effective \
                 antidote is actively seeking disconfirming evidence — asking 'what would \
                 prove me wrong?' and genuinely wanting to find out.",

                "The Big Five personality traits — Openness, Conscientiousness, Extraversion, \
                 Agreeableness, and Neuroticism (OCEAN) — have emerged as the most robust, \
                 cross-culturally replicated model of personality structure. Traits are \
                 relatively stable over the lifespan but can shift gradually through major \
                 life events and intentional effort. Conscientiousness is the trait most \
                 strongly predictive of academic and occupational success.",

                "Flow states, described by Csikszentmihalyi, occur when the challenge of \
                 an activity matches the person's skill level — both are high. Time \
                 distorts, self-consciousness disappears, and the activity becomes its own \
                 reward. Flow requires clear goals, immediate feedback, and a sense of \
                 control. It is not relaxation — it is intense engagement with a demanding \
                 task. Flow is strongly correlated with happiness and life satisfaction.",

                "The hedonic treadmill is the observation that people quickly return to a \
                 baseline level of happiness after positive or negative events. Lottery \
                 winners are no happier than the general population after about a year; \
                 paraplegics adapt and report levels of well-being close to their baseline. \
                 The treadmill explains why more money, possessions, or achievements do not \
                 produce lasting happiness. Activities that produce flow, relationships that \
                 provide connection, and a sense of meaning are more durable sources of \
                 well-being than hedonic pleasures.",

                "Mirror neurons fire both when performing an action and when observing \
                 someone else perform the same action. Discovered in macaque monkeys in the \
                 1990s, they have been proposed as a neural basis for empathy, imitation, \
                 and language acquisition. The human mirror neuron system is more complex \
                 and its exact role is still debated, but the core insight — that the brain \
                 simulates observed actions — has influenced fields from neuroscience to \
                 aesthetics.",

                "Growth mindset, a concept from Carol Dweck, is the belief that abilities \
                 can be developed through effort, learning, and persistence. The alternative \
                 — a fixed mindset — treats ability as innate and unchangeable. Students with \
                 a growth mindset respond to failure by working harder and trying different \
                 strategies; those with a fixed mindset interpret failure as evidence of a \
                 permanent limitation. The intervention is simple but powerful: praise \
                 effort and strategy, not intelligence or talent.",

                "The bystander effect describes the reduced likelihood of helping in an \
                 emergency when other people are present. The more bystanders, the less \
                 likely any individual is to act — each person assumes someone else will \
                 take responsibility (diffusion of responsibility). The effect was \
                 dramatically demonstrated after the 1964 murder of Kitty Genovese, though \
                 later reporting corrected some details of that case. Knowing about the \
                 bystander effect makes you less susceptible to it: in an emergency, \
                 point to a specific person and give a clear instruction.",
            ],
        },

        // ── Education ───────────────────────────────────────────────────
        DomainPool {
            name: "Education",
            tags: &["education", "learning", "teaching", "knowledge"],
            title_fragments: &[
                "How to learn", "The", "A guide to", "Understanding",
                "Why", "Teaching", "The art of", "Principles of",
            ],
            paragraphs: &[
                "Spaced repetition exploits the spacing effect — the finding that information \
                 is retained better when study sessions are spaced out over time rather than \
                 crammed. Software like Anki schedules reviews at optimal intervals based on \
                 your performance. Items you find easy are shown less often; difficult items \
                 reappear until they become easy. The algorithm implements a simple principle: \
                 review just before you would forget.",

                "Active recall — testing yourself on material rather than re-reading it — is \
                 significantly more effective for long-term retention. Re-reading creates an \
                 illusion of fluency: the material feels familiar, so you assume you know it. \
                 But familiarity is not understanding. The effort of retrieving information \
                 from memory strengthens the neural pathways that make future retrieval easier. \
                 Close the book and try to explain the concept from memory.",

                "The Feynman Technique is a method for learning and checking understanding: \
                 explain the concept in simple language as if teaching it to someone with no \
                 background in the subject. When you struggle to explain something clearly, \
                 you have found a gap in your understanding. Return to the source material, \
                 fill the gap, and try the explanation again. The technique works because \
                 teaching forces you to confront what you do not actually know.",

                "Interleaving — mixing different topics or problem types within a study \
                 session rather than blocking them — improves the ability to discriminate \
                 between problem types and select the right approach. Blocked practice \
                 produces better performance during the session, creating an illusion of \
                 mastery, but interleaved practice produces better retention and transfer. \
                 The effort of switching contexts and retrieving the right method strengthens \
                 learning.",

                "Bloom's Taxonomy classifies learning objectives into six levels: Remember, \
                 Understand, Apply, Analyse, Evaluate, and Create. Most formal education \
                 focuses heavily on the first two levels and neglects the higher ones. \
                 Self-directed learners can use the taxonomy to check whether they are \
                 engaging with material deeply — are you just recalling facts, or are you \
                 using them to create something new?",

                "Metacognition — thinking about your own thinking — is one of the strongest \
                 predictors of academic success. It includes planning (what do I need to \
                 learn?), monitoring (am I understanding this?), and evaluating (did my \
                 approach work?). Students who regularly ask themselves these questions \
                 outperform those with equal raw ability who do not. Metacognition can be \
                 taught and improved with practice.",

                "The zone of proximal development, from Vygotsky, is the gap between what \
                 a learner can do independently and what they can do with guidance. Effective \
                 instruction targets this zone — the material should be challenging enough to \
                 require effort but not so difficult that it is impossible. Scaffolding \
                 (temporary support that is gradually removed as competence grows) is the \
                 practical implementation of this principle.",

                "Intrinsic motivation — doing something because it is inherently interesting \
                 or satisfying — produces deeper engagement and better learning outcomes than \
                 extrinsic rewards (grades, money, praise). Self-determination theory \
                 identifies three psychological needs that support intrinsic motivation: \
                 autonomy (choice and control), competence (mastery and growth), and \
                 relatedness (connection to others). Learning environments that satisfy these \
                 needs produce more motivated, persistent learners.",

                "Transfer of learning — applying knowledge from one context to another — is \
                 the ultimate goal of education, but it happens far less automatically than \
                 we assume. Knowledge is often 'stuck' to the context in which it was learned. \
                 Teaching for transfer requires using multiple examples, varying the context, \
                 and explicitly pointing out the underlying principles that connect different \
                 surface features.",

                "Deliberate practice, as studied by Anders Ericsson, is a specific form of \
                 practice: it targets performance at the edge of current ability, provides \
                 immediate feedback, allows for repetition with correction, and requires \
                 intense focus. It is not the same as mindless repetition. Ten thousand \
                 hours of the wrong kind of practice produces expert-level mediocrity. \
                 Deliberate practice is mentally demanding and can typically be sustained \
                 for only a few hours per day.",

                "The testing effect, also called retrieval practice, is one of the most \
                 robust findings in cognitive psychology: taking a test on material improves \
                 long-term retention more than re-studying it for an equivalent amount of \
                 time. The effect is stronger when feedback is provided and when the test \
                 requires production (short answer, essay) rather than recognition (multiple \
                 choice). Tests are not just assessment tools — they are learning events.",
            ],
        },
    ]
}

#[cfg(test)]
mod domain_pool_tests {
    #[test]
    fn domain_pools_non_empty_and_has_technology() {
        let pools = super::domain_pools();
        assert!(!pools.is_empty());
        assert!(pools.iter().any(|p| p.name == "Technology"));
    }
}
